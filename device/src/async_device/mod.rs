//! Asynchronous Device using Rust async-await for driving the state machine,
//! and allowing asynchronous radio implementations.
use super::mac::uplink::Uplink;

pub use super::{region, region::Region, JoinMode, SendData, Timings};
use super::{
    region::{Frame, Window},
    Credentials,
};
use core::marker::PhantomData;
use futures::{future::select, future::Either, pin_mut};
use generic_array::{typenum::U256, GenericArray};
use heapless::Vec;
use lorawan::{
    self,
    creator::DataPayloadCreator,
    keys::CryptoFactory,
    maccommands::SerializableMacCommand,
    parser::DevAddr,
    parser::{parse_with_factory as lorawan_parse, *},
};
use rand_core::RngCore;

type DevNonce = lorawan::parser::DevNonce<[u8; 2]>;
pub use crate::region::DR;
use crate::{private::Sealed, radio::types::RadioBuffer, AppSKey, GetRng, NewSKey};
#[cfg(feature = "external-lora-phy")]
/// provide the radio through the external lora-phy crate
pub mod lora_radio;
pub mod radio;

#[cfg(test)]
mod test;

use core::cmp::min;

/// Type-level version of the [`None`] variant
pub struct NoneT;

/// Type-level Option representing a type that can either implement [`RngCore`] or be [`NoneT`].
/// This trait is an implementation detail and should not be implemented outside this crate.
#[doc(hidden)]
pub trait OptionalRng: Sealed {}

impl Sealed for NoneT {}
impl OptionalRng for NoneT {}

impl<T: RngCore> Sealed for T {}
impl<T: RngCore> OptionalRng for T {}

/// Representation of the physical radio + RNG. Two variants may be constructed through [`Device`].
/// Either:
/// * `R` implements [`RngCore`], or
/// * `G` implements [`RngCore`].
///
/// This allows for seamless functionality with either RNG variant and is an implementation detail.
/// Users are not expected to construct [`Phy`] directly. Use the constructors for [`Device`]
/// instead.
pub struct Phy<R, G: OptionalRng> {
    radio: R,
    rng: G,
}

impl<R: RngCore> Sealed for Phy<R, NoneT> {}
impl<R: RngCore> GetRng for Phy<R, NoneT> {
    type RNG = R;
    fn get_rng(&mut self) -> &mut Self::RNG {
        &mut self.radio
    }
}

impl<R, G: RngCore> Sealed for Phy<R, G> {}
impl<R, G> GetRng for Phy<R, G>
where
    R: radio::PhyRxTx + Timings,
    G: RngCore,
{
    type RNG = G;
    fn get_rng(&mut self) -> &mut Self::RNG {
        &mut self.rng
    }
}

/// Type representing a LoRaWAN cabable device. A device is bound to the following types:
/// - R: An asynchronous radio implementation
/// - T: An asynchronous timer implementation
/// - C: A CryptoFactory implementation
/// - RNG: A random number generator implementation. This is optional depending on whether you
///   construct [`Device`]
/// with the `new` or `new_with_builtin_rng` methods.
pub struct Device<R, C, T, G, const N: usize = 256>
where
    R: radio::PhyRxTx + Timings,
    T: radio::Timer,
    C: CryptoFactory + Default,
    G: OptionalRng,
    Phy<R, G>: GetRng,
{
    crypto: PhantomData<C>,
    region: region::Configuration,
    phy: Phy<R, G>,
    timer: T,
    session: Option<SessionData>,
    mac: Uplink,
    radio_buffer: RadioBuffer<N>,
    datarate: DR,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub enum Error<R> {
    Radio(R),
    NetworkNotJoined,
    UnableToPreparePayload(&'static str),
    InvalidDevAddr,
    RxTimeout,
    SessionExpired,
    InvalidMic,
    UnableToDecodePayload(&'static str),
}

impl<R, C, T, const N: usize> Device<R, C, T, NoneT, N>
where
    R: radio::PhyRxTx + Timings + RngCore,
    C: CryptoFactory + Default,
    T: radio::Timer,
{
    /// Create a new instance of [`Device`] with a LoRa chip with a builtin RNG.
    /// This means that `radio` should implement [`rand_core::RngCore`].
    pub fn new_with_builtin_rng(
        region: region::Configuration,
        radio: R,
        timer: T,
    ) -> Device<R, C, T, NoneT, N> {
        Device::new_with_session(region, radio, timer, NoneT, None)
    }
}

impl<R, C, T, G, const N: usize> Device<R, C, T, G, N>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
    T: radio::Timer,
    G: RngCore,
{
    /// Create a new instance of [`Device`] with a RNG external to the LoRa chip.
    /// See also [`new_with_builtin_rng`](Self::new_with_builtin_rng)
    pub fn new(region: region::Configuration, radio: R, timer: T, rng: G) -> Device<R, C, T, G, N> {
        Device::new_with_session(region, radio, timer, rng, None)
    }
}

#[allow(dead_code)]
impl<R, C, T, G, const N: usize> Device<R, C, T, G, N>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
    T: radio::Timer,
    G: OptionalRng,
    Phy<R, G>: GetRng,
{
    pub fn new_with_session(
        region: region::Configuration,
        radio: R,
        timer: T,
        rng: G,
        session: Option<SessionData>,
    ) -> Self {
        Self {
            crypto: PhantomData,
            phy: Phy { radio, rng },
            session,
            mac: Uplink::default(),
            radio_buffer: RadioBuffer::new(),
            timer,
            datarate: region.get_default_datarate(),
            region,
        }
    }

    pub fn get_session(&mut self) -> &Option<SessionData> {
        &self.session
    }

    pub fn get_region(&mut self) -> &region::Configuration {
        &self.region
    }

    pub fn get_radio(&mut self) -> &R {
        &self.phy.radio
    }

    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.phy.radio
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
    pub async fn join(&mut self, join_mode: &JoinMode) -> Result<(), Error<R::PhyError>> {
        match join_mode {
            JoinMode::OTAA { deveui, appeui, appkey } => {
                let credentials = Credentials::new(*appeui, *deveui, *appkey);

                // Prepare the buffer with the join payload
                let (devnonce, tx_config) = credentials
                    .create_join_request::<C, <Phy<R, G> as GetRng>::RNG, N>(
                        &mut self.region,
                        self.phy.get_rng(),
                        self.datarate,
                        &mut self.radio_buffer,
                    );

                // Transmit the join payload
                let ms = self
                    .phy
                    .radio
                    .tx(tx_config, self.radio_buffer.as_ref())
                    .await
                    .map_err(Error::Radio)?;

                // Receive join response within RX window
                self.timer.reset();
                self.rx_with_timeout(&Frame::Join, ms).await?;

                // Parse join response
                match lorawan_parse(self.radio_buffer.as_mut(), C::default()) {
                    Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(encrypted))) => {
                        let decrypt = encrypted.decrypt(&credentials.appkey().0);
                        self.region.process_join_accept(&decrypt);
                        if decrypt.validate_mic(&credentials.appkey().0) {
                            let data = SessionData::derive_new(&decrypt, devnonce, &credentials);
                            self.session.replace(data);
                            Ok(())
                        } else {
                            Err(Error::InvalidMic)
                        }
                    }
                    Err(err) => Err(Error::UnableToDecodePayload(err)),
                    _ => Err(Error::UnableToDecodePayload("")),
                }
            }
            JoinMode::ABP { newskey, appskey, devaddr } => {
                self.session.replace(SessionData::new(*newskey, *appskey, *devaddr));
                Ok(())
            }
        }
    }

    /// Send data on a given port with the expected confirmation. The returned future completes
    /// when the data have been sent successfully, or an error has occured.
    pub async fn send(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) -> Result<(), Error<R::PhyError>> {
        self.send_recv_internal(data, fport, confirmed, None).await?;
        Ok(())
    }

    /// Send data on a given port with the expected confirmation. If downlink data is provided, the
    /// data is copied into the provided byte slice.
    ///
    /// The returned future completes when the data have been sent successfully and downlink data
    /// have been copied into the provided buffer, or an error has occured.
    pub async fn send_recv(
        &mut self,
        data: &[u8],
        rx: &mut [u8],
        fport: u8,
        confirmed: bool,
    ) -> Result<usize, Error<R::PhyError>> {
        self.send_recv_internal(data, fport, confirmed, Some(rx)).await
    }

    /// Send data and fill rx buffer if provided
    async fn send_recv_internal(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
        rx: Option<&mut [u8]>,
    ) -> Result<usize, Error<R::PhyError>> {
        if self.session.is_none() {
            return Err(Error::NetworkNotJoined);
        }

        // Prepare transmission buffer
        let _ = self.prepare_buffer(data, fport, confirmed)?;

        // Send data
        let tx_config =
            self.region.create_tx_config(self.phy.get_rng(), self.datarate, &Frame::Data);

        // Unless the same frame is to be retransmitted (see NbTrans parameter of LinkADRReq
        // command, LoRaWAN spec 1.0.2 section 5.2 for retransmissions), FCnt must be
        // incremented on each transmission.
        self.session.as_mut().ok_or(Error::NetworkNotJoined)?.fcnt_up_increment();

        // Transmit our data packet
        let ms =
            self.phy.radio.tx(tx_config, self.radio_buffer.as_ref()).await.map_err(Error::Radio)?;

        // Wait for received data within window
        self.timer.reset();
        self.rx_with_timeout(&Frame::Data, ms).await?;

        // Handle received data
        if let Some(ref mut session_data) = self.session {
            // Parse payload and copy into user bufer is provided
            match lorawan_parse(self.radio_buffer.as_mut(), C::default()) {
                Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) => {
                    if session_data.devaddr() == &encrypted_data.fhdr().dev_addr() {
                        let fcnt = encrypted_data.fhdr().fcnt() as u32;
                        let confirmed = encrypted_data.is_confirmed();
                        if encrypted_data.validate_mic(&session_data.newskey().0, fcnt)
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
                                    Some(&session_data.newskey().0),
                                    Some(&session_data.appskey().0),
                                    session_data.fcnt_down,
                                )
                                .unwrap();

                            self.mac.handle_downlink_macs(
                                &mut self.region,
                                &mut decrypted.fhdr().fopts(),
                            );

                            if confirmed {
                                self.mac.set_downlink_confirmation();
                            }

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
    fn prepare_buffer(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) -> Result<u32, Error<R::PhyError>> {
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

                let mut fctrl = FCtrl(0x0, true);
                if self.mac.confirms_downlink() {
                    fctrl.set_ack();
                    self.mac.clear_downlink_confirmation();
                }

                phy.set_confirmed(confirmed)
                    .set_fctrl(&fctrl)
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
                    data,
                    dyn_cmds.as_slice(),
                    &session_data.newskey().0,
                    &session_data.appskey().0,
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
    async fn rx_with_timeout(
        &mut self,
        frame: &Frame,
        window_delay: u32,
    ) -> Result<(), Error<R::PhyError>> {
        let num_read = self.rx_with_timeout_inner(frame, window_delay).await?;
        self.radio_buffer.inc(num_read);
        Ok(())
    }

    async fn rx_with_timeout_inner(
        &mut self,
        frame: &Frame,
        window_delay: u32,
    ) -> Result<usize, Error<R::PhyError>> {
        // The initial window configuration uses window 1 adjusted by window_delay and radio offset
        let rx1_start_delay = (self.region.get_rx_delay(frame, &Window::_1) as i32
            + window_delay as i32
            + self.phy.radio.get_rx_window_offset_ms()) as u32;

        let rx1_end_delay = rx1_start_delay + self.phy.radio.get_rx_window_duration_ms();

        let rx2_start_delay = (self.region.get_rx_delay(frame, &Window::_2) as i32
            + window_delay as i32
            + self.phy.radio.get_rx_window_offset_ms()) as u32;

        self.radio_buffer.clear();
        // Wait until RX1 window opens
        self.timer.at(rx1_start_delay.into()).await;

        // RX1
        enum Rx1 {
            Rx(usize),
            Timeout(u32),
        }
        let response = {
            // Prepare for RX using correct configuration
            let rx_config = self.region.get_rx_config(self.datarate, frame, &Window::_1);
            // Cap window duration so RX2 can start on time
            let window_duration = min(rx1_end_delay, rx2_start_delay);

            // Pass the full radio buffer slice to RX
            self.phy.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
            let rx_fut = self.phy.radio.rx(self.radio_buffer.as_raw_slice());
            let timeout_fut = self.timer.at(window_duration.into());

            pin_mut!(rx_fut);
            pin_mut!(timeout_fut);
            // Wait until either RX is complete or if we've reached window close
            match select(rx_fut, timeout_fut).await {
                // RX is complete!
                Either::Left((r, timeout_fut)) => match r {
                    Ok((sz, _q)) => Rx1::Rx(sz),
                    // Ignore errors or timeouts and wait until the RX2 window is ready.
                    // Setting timeout to 0 ensures that `window_duration != rx2_start_delay`
                    _ => {
                        timeout_fut.await;
                        Rx1::Timeout(0)
                    }
                },
                // Timeout! Prepare for the next window.
                Either::Right(_) => Rx1::Timeout(window_duration),
            }
        };

        match response {
            Rx1::Rx(sz) => {
                self.phy.radio.low_power().await.map_err(Error::Radio)?;
                return Ok(sz);
            }
            Rx1::Timeout(window_duration) => {
                // If the window duration was the same as the RX2 start delay, we can skip settings the
                // radio to lower power and arming the the timer
                if window_duration != rx2_start_delay {
                    self.phy.radio.low_power().await.map_err(Error::Radio)?;
                    self.timer.at(rx2_start_delay.into()).await;
                }
            }
        }

        // RX2
        let response = {
            // Prepare for RX using correct configuration
            let rx_config = self.region.get_rx_config(self.datarate, frame, &Window::_2);
            let window_duration = self.phy.radio.get_rx_window_duration_ms();

            // Pass the full radio buffer slice to RX
            self.phy.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
            let rx_fut = self.phy.radio.rx(self.radio_buffer.as_raw_slice());
            let timeout_fut = self.timer.delay_ms(window_duration.into());

            pin_mut!(rx_fut);
            pin_mut!(timeout_fut);
            // Wait until either RX is complete or if we've reached window close
            match select(rx_fut, timeout_fut).await {
                // RX is complete!
                Either::Left((Ok((sz, _q)), _)) => Ok(sz),
                // Timeout or other RX error.
                _ => Err(Error::RxTimeout),
            }
        };
        self.phy.radio.low_power().await.map_err(Error::Radio)?;
        response
    }
}

/// Contains data for the current session
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SessionData {
    newskey: NewSKey,
    appskey: AppSKey,
    devaddr: DevAddr<[u8; 4]>,
    fcnt_up: u32,
    fcnt_down: u32,
}

impl SessionData {
    pub fn derive_new<T: core::convert::AsRef<[u8]>, F: lorawan::keys::CryptoFactory>(
        decrypt: &DecryptedJoinAcceptPayload<T, F>,
        devnonce: DevNonce,
        credentials: &Credentials,
    ) -> SessionData {
        Self::new(
            NewSKey(decrypt.derive_newskey(&devnonce, &credentials.appkey().0)),
            AppSKey(decrypt.derive_appskey(&devnonce, &credentials.appkey().0)),
            decrypt.dev_addr().to_owned(),
        )
    }

    pub fn new(newskey: NewSKey, appskey: AppSKey, devaddr: DevAddr<[u8; 4]>) -> SessionData {
        SessionData { newskey, appskey, devaddr, fcnt_up: 0, fcnt_down: 0 }
    }

    pub fn newskey(&self) -> &NewSKey {
        &self.newskey
    }

    pub fn appskey(&self) -> &AppSKey {
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
