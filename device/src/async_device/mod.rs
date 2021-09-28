//! Asynchronous Device using Rust async-await for driving the state machine,
//! and allowing asynchronous radio implementations.
use super::mac::Mac;

use super::{
    radio::TxConfig,
    region::{Frame, Window},
    Credentials,
};
pub use super::{region, region::Region, types::*, JoinMode, SendData, Timings};
use core::marker::PhantomData;
use futures::{future::select, future::Either, pin_mut};
use generic_array::{typenum::U256, GenericArray};
use heapless::Vec;
use lorawan_encoding::{
    self,
    creator::DataPayloadCreator,
    creator::JoinRequestCreator,
    keys::{CryptoFactory, AES128},
    maccommands::SerializableMacCommand,
    parser::DevAddr,
    parser::{parse_with_factory as lorawan_parse, *},
};
use rand_core::RngCore;

type DevNonce = lorawan_encoding::parser::DevNonce<[u8; 2]>;
pub use crate::region::DR;
use radio::RadioBuffer;
pub mod radio;

/// Type representing a LoRaWAN cabable device. A device is bound to the following types:
/// - R: An asynchronous radio implementation
/// - T: An asynchronous timer implementation
/// - C: A CryptoFactory implementation
/// - RNG: A random number generator implementation
pub struct Device<'a, R, C, T, RNG>
where
    R: radio::PhyRxTx + Timings,
    T: radio::Timer,
    C: CryptoFactory + Default,
    RNG: RngCore,
{
    crypto: PhantomData<C>,
    region: region::Configuration,
    radio: R,
    timer: T,
    rng: RNG,
    session: Option<SessionData>,
    mac: Mac,
    radio_buffer: RadioBuffer<'a>,
    datarate: DR,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<R: radio::PhyRxTx> {
    Radio(R::PhyError),
    NetworkNotJoined,
    UnableToPreparePayload(&'static str),
    InvalidDevAddr,
    RxTimeout,
    SessionExpired,
    InvalidMic,
    UnableToDecodePayload(&'static str),
}

#[allow(dead_code)]
impl<'a, R, C, T, RNG> Device<'a, R, C, T, RNG>
where
    R: radio::PhyRxTx + Timings + 'a,
    C: CryptoFactory + Default,
    T: radio::Timer,
    RNG: RngCore,
{
    pub fn new(
        region: region::Configuration,
        radio: R,
        timer: T,
        rng: RNG,
        radio_buffer: &'a mut [u8],
    ) -> Self {
        Self {
            crypto: PhantomData::default(),
            radio,
            session: None,
            mac: Mac::default(),
            radio_buffer: RadioBuffer::new(radio_buffer),
            timer,
            rng,
            datarate: region.get_default_datarate(),
            region,
        }
    }

    /// Retrieve the current data rate being used by this device.
    pub fn get_datarate(&mut self) -> region::DR {
        self.datarate
    }

    /// Set the data rate being used by this device. This overrides the region default.
    pub fn set_datarate(&mut self, datarate: region::DR) {
        self.datarate = datarate;
    }

    /// Join the LoRaWAN network asynchronusly. The returned future completes when
    /// the LoRaWAN network has been joined successfully, or an error has occured.
    ///
    /// Repeatedly calling join using OTAA will result in a new LoRaWAN session to be created.
    pub async fn join(&mut self, join_mode: &JoinMode) -> Result<(), Error<R>> {
        match join_mode {
            JoinMode::OTAA {
                deveui,
                appeui,
                appkey,
            } => {
                let credentials = Credentials::new(*appeui, *deveui, *appkey);

                // Prepare the buffer with the join payload
                let (devnonce, tx_config) = self.create_join_request(&credentials);

                // Transmit the join payload
                let ms = self
                    .radio
                    .tx(tx_config, self.radio_buffer.as_ref())
                    .await
                    .map_err(|e| Error::Radio(e))?;

                // Receive join response within RX window
                self.rx_with_timeout(ms).await?;

                // Parse join response
                match lorawan_parse(self.radio_buffer.as_mut(), C::default()) {
                    Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(encrypted))) => {
                        let decrypt = encrypted.decrypt(credentials.appkey());
                        if decrypt.validate_mic(credentials.appkey()) {
                            let data = SessionData::derive_new(&decrypt, devnonce, &credentials);
                            self.session.replace(data);
                            Ok(())
                        } else {
                            Err(Error::InvalidMic)
                        }
                    }
                    _ => Err(Error::UnableToDecodePayload("")),
                }
            }
            JoinMode::ABP {
                newskey,
                appskey,
                devaddr,
            } => {
                self.session
                    .replace(SessionData::new(*newskey, *appskey, *devaddr));
                Ok(())
            }
        }
    }

    /// Send data on a given port with the expected confirmation. The returned future completes
    /// when the data have been sent successfully, or an error has occured.
    pub async fn send(&mut self, data: &[u8], fport: u8, confirmed: bool) -> Result<(), Error<R>> {
        self.send_recv_internal(data, fport, confirmed, None)
            .await?;
        Ok(())
    }

    /// Send data on a given port with the expected confirmation. If downlink data is provided, the data is
    /// copied into the provided byte slice.
    ///
    /// The returned future completes when the data have been sent successfully and downlink data have been
    /// copied into the provided buffer, or an error has occured.
    pub async fn send_recv(
        &mut self,
        data: &[u8],
        rx: &mut [u8],
        fport: u8,
        confirmed: bool,
    ) -> Result<usize, Error<R>> {
        self.send_recv_internal(data, fport, confirmed, Some(rx))
            .await
    }

    /// Send data and fill rx buffer if provided
    async fn send_recv_internal(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
        rx: Option<&mut [u8]>,
    ) -> Result<usize, Error<R>> {
        if self.session.is_none() {
            return Err(Error::NetworkNotJoined);
        }

        // Prepare transmission buffer
        let _ = self.prepare_buffer(data, fport, confirmed)?;

        // Send data
        let random = self.rng.next_u32();
        let tx_config = self
            .region
            .create_tx_config(random as u8, self.datarate, &Frame::Data);

        // Transmit our data packet
        let ms = self
            .radio
            .tx(tx_config, self.radio_buffer.as_ref())
            .await
            .map_err(|e| Error::Radio(e))?;

        // Wait for received data within window
        self.rx_with_timeout(ms).await?;

        // Handle received data
        if let Some(ref mut session_data) = self.session {
            // Parse payload and copy into user bufer is provided
            match lorawan_parse(self.radio_buffer.as_mut(), C::default()) {
                Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) => {
                    if session_data.devaddr() == &encrypted_data.fhdr().dev_addr() {
                        let fcnt = encrypted_data.fhdr().fcnt() as u32;
                        if encrypted_data.validate_mic(&session_data.newskey(), fcnt)
                            && (fcnt > session_data.fcnt_down || fcnt == 0)
                        {
                            session_data.fcnt_down = fcnt;
                            // increment the FcntUp since we have received
                            // downlink - only reason to not increment
                            // is if confirmed frame is sent and no
                            // confirmation (ie: downlink) occurs
                            session_data.fcnt_up_increment();

                            // * the decrypt will always work when we have verified MIC previously
                            let decrypted = encrypted_data
                                .decrypt(
                                    Some(&session_data.newskey()),
                                    Some(&session_data.appskey()),
                                    session_data.fcnt_down,
                                )
                                .unwrap();

                            self.mac.handle_downlink_macs(
                                &mut self.region,
                                &mut decrypted.fhdr().fopts(),
                            );

                            match decrypted.frm_payload() {
                                Ok(FRMPayload::MACCommands(mac_cmds)) => {
                                    self.mac.handle_downlink_macs(
                                        &mut self.region,
                                        &mut mac_cmds.mac_commands(),
                                    );
                                    Ok(0)
                                }
                                Ok(FRMPayload::Data(rx_data)) => {
                                    if let Some(rx) = rx {
                                        let to_copy = core::cmp::min(rx.len(), rx_data.len());
                                        rx[0..to_copy].copy_from_slice(&rx_data[0..to_copy]);
                                        Ok(to_copy)
                                    } else {
                                        Ok(0)
                                    }
                                }
                                Ok(FRMPayload::None) => Ok(0),
                                Err(_) => Err(Error::UnableToDecodePayload("")),
                            }
                        } else {
                            Err(Error::InvalidMic)
                        }
                    } else {
                        Err(Error::InvalidDevAddr)
                    }
                }
                Ok(_) => Err(Error::UnableToDecodePayload("")),
                Err(e) => Err(Error::UnableToDecodePayload(e)),
            }
        } else {
            Err(Error::NetworkNotJoined)
        }
    }

    // Prepare radio buffer with data using session state
    fn prepare_buffer(&mut self, data: &[u8], fport: u8, confirmed: bool) -> Result<u32, Error<R>> {
        match self.session {
            Some(ref session_data) => {
                // check if FCnt is used up
                if session_data.fcnt_up() == (0xFFFF + 1) {
                    // signal that the session is expired
                    return Err(Error::SessionExpired);
                }
                let fcnt = session_data.fcnt_up();
                let mut phy: DataPayloadCreator<GenericArray<u8, U256>, C> =
                    DataPayloadCreator::default();
                phy.set_confirmed(confirmed)
                    .set_f_port(fport)
                    .set_dev_addr(*session_data.devaddr())
                    .set_fcnt(fcnt);

                let mut cmds = Vec::new();
                self.mac.get_cmds(&mut cmds);

                let mut dyn_cmds: Vec<&dyn SerializableMacCommand, 8> = Vec::new();

                for cmd in &cmds {
                    if let Err(_e) = dyn_cmds.push(cmd) {
                        panic!("dyn_cmds too small compared to cmds")
                    }
                }

                match phy.build(
                    &data,
                    dyn_cmds.as_slice(),
                    session_data.newskey(),
                    session_data.appskey(),
                ) {
                    Ok(packet) => {
                        self.radio_buffer.clear();
                        self.radio_buffer.extend_from_slice(packet).unwrap();
                        Ok(fcnt)
                    }
                    Err(e) => Err(Error::UnableToPreparePayload(e)),
                }
            }
            None => Err(Error::NetworkNotJoined),
        }
    }

    /// Attempt to receive data within RX1 and RX2 windows. This function will populate the
    /// provided buffer with data if received. Will return a RxTimeout error if no RX within
    /// the windows.
    async fn rx_with_timeout(&mut self, window_delay: u32) -> Result<(), Error<R>> {
        // The initial window configuration uses window 1 adjusted by window_delay and radio offset
        let mut window_open = (self.region.get_rx_delay(&Frame::Join, &Window::_1) as i32
            + window_delay as i32
            + self.radio.get_rx_window_offset_ms()) as u32;
        let mut window = Window::_1;

        let time_between_windows = self.region.get_rx_delay(&Frame::Join, &Window::_2)
            - self.region.get_rx_delay(&Frame::Join, &Window::_1);

        // Prepare buffer for receiption
        let response: Result<usize, Error<R>>;
        self.radio_buffer.clear();
        loop {
            // Wait until RX window opens
            self.timer.delay_ms(window_open.into()).await;

            // Calculate the time until window closes
            let window_close: u32 = match window {
                // RxWindow1 one must timeout before RxWindow2
                Window::_1 => {
                    if time_between_windows > self.radio.get_rx_window_duration_ms() {
                        window_open + self.radio.get_rx_window_duration_ms()
                    } else {
                        window_open + time_between_windows
                    }
                }
                // RxWindow2 can last however long
                Window::_2 => window_open + self.radio.get_rx_window_duration_ms(),
            };

            // Prepare for RX using correct configuration
            let rx_config = self
                .region
                .get_rx_config(self.datarate, &Frame::Join, &window);

            // Pass the full radio buffer slice to RX
            let rx_fut = self.radio.rx(rx_config, self.radio_buffer.as_raw_slice());
            let timeout_fut = self.timer.delay_ms(window_close.into());

            pin_mut!(rx_fut);
            pin_mut!(timeout_fut);
            // Wait until either RX is complete or if we've reached window close
            match select(rx_fut, timeout_fut).await {
                // RX is complete!
                Either::Left((r, _)) => match r {
                    Ok((sz, _q)) => {
                        response = Ok(sz);
                        break;
                    }
                    Err(e) => {
                        response = Err(Error::Radio(e));
                        break;
                    }
                },
                // Timeout! Jumpt to next window or report timeout
                Either::Right(_) => match window {
                    Window::_1 => {
                        window = Window::_2;
                        window_open = window_open + time_between_windows;
                    }
                    Window::_2 => {
                        response = Err(Error::RxTimeout.into());
                        break;
                    }
                },
            }
        }

        // Throw error down;
        let rx_len = response?;
        if rx_len > 0 {
            // Ensure radio buffer is consistent after RX
            self.radio_buffer.inc(rx_len);
        }
        Ok(())
    }

    /// Prepare a join request to be sent. This populates the radio buffer with the request to be
    /// sent, and returns the radio config to use for transmitting.
    fn create_join_request(&mut self, creds: &Credentials) -> (DevNonce, TxConfig) {
        let mut random = self.rng.next_u32();
        // use lowest 16 bits for devnonce
        let devnonce_bytes = random as u16;

        self.radio_buffer.clear();

        let mut phy: JoinRequestCreator<[u8; 23], C> = JoinRequestCreator::default();

        let devnonce = [devnonce_bytes as u8, (devnonce_bytes >> 8) as u8];

        phy.set_app_eui(EUI64::new(creds.appeui()).unwrap())
            .set_dev_eui(EUI64::new(creds.deveui()).unwrap())
            .set_dev_nonce(&devnonce);
        let vec = phy.build(&creds.appkey()).unwrap();

        let devnonce_copy = DevNonce::new(devnonce).unwrap();

        self.radio_buffer.extend_from_slice(vec).unwrap();

        // we'll use the rest for frequency and subband selection
        random >>= 16;
        (
            devnonce_copy,
            self.region
                .create_tx_config(random as u8, self.datarate, &Frame::Join),
        )
    }
}

/// Contains data for the current session
struct SessionData {
    newskey: AES128,
    appskey: AES128,
    devaddr: DevAddr<[u8; 4]>,
    fcnt_up: u32,
    fcnt_down: u32,
}

impl SessionData {
    pub fn derive_new<T: core::convert::AsRef<[u8]>, F: lorawan_encoding::keys::CryptoFactory>(
        decrypt: &DecryptedJoinAcceptPayload<T, F>,
        devnonce: DevNonce,
        credentials: &Credentials,
    ) -> SessionData {
        Self::new(
            decrypt.derive_newskey(&devnonce, credentials.appkey()),
            decrypt.derive_appskey(&devnonce, credentials.appkey()),
            DevAddr::new([
                decrypt.dev_addr().as_ref()[0],
                decrypt.dev_addr().as_ref()[1],
                decrypt.dev_addr().as_ref()[2],
                decrypt.dev_addr().as_ref()[3],
            ])
            .unwrap(),
        )
    }

    pub fn new(newskey: AES128, appskey: AES128, devaddr: DevAddr<[u8; 4]>) -> SessionData {
        SessionData {
            newskey,
            appskey,
            devaddr,
            fcnt_up: 0,
            fcnt_down: 0,
        }
    }

    pub fn newskey(&self) -> &AES128 {
        &self.newskey
    }

    pub fn appskey(&self) -> &AES128 {
        &self.appskey
    }

    pub fn devaddr(&self) -> &DevAddr<[u8; 4]> {
        &self.devaddr
    }

    pub fn fcnt_up(&self) -> u32 {
        self.fcnt_up
    }

    pub fn fcnt_up_increment(&mut self) {
        self.fcnt_up += 1;
    }
}
