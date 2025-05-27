//! LoRaWAN device which uses async-await for driving the protocol state against pin and timer events,
//! allowing for asynchronous radio implementations. Requires the `async` feature.
use super::mac::{self, FcntDown, Frame, Mac, Window};
pub use super::{
    mac::{NetworkCredentials, SendData, Session},
    region::{self, Region},
    Downlink, JoinMode,
};
use heapless::Vec;
use rand_core::RngCore;

pub use crate::region::DR;
use crate::{
    radio::{RadioBuffer, RxConfig},
    rng,
};

pub mod radio;
use lorawan::default_crypto::DefaultFactory;

#[cfg(feature = "embassy-time")]
mod embassy_time;
#[cfg(feature = "embassy-time")]
pub use embassy_time::EmbassyTimer;

#[cfg(feature = "multicast")]
use crate::mac::multicast;
#[cfg(feature = "multicast")]
pub use lorawan::{
    keys::{AppKey, AppSKey, GenAppKey, McAppSKey, McNetSKey, McRootKey},
    parser::McAddr,
};

#[cfg(feature = "multicast")]
#[derive(Debug, Clone, Copy)]
/// Multicast Groups range from 0 to 3.
pub enum McGroup {
    _0,
    _1,
    _2,
    _3,
}

#[cfg(test)]
mod test;

use self::radio::RxStatus;

/// Type representing a LoRaWAN capable device.
///
/// A device is bound to the following types:
/// - R: An asynchronous radio implementation
/// - T: An asynchronous timer implementation
/// - RNG: A random number generator implementation. An external RNG may be provided, or you may use a builtin PRNG by
///   providing a random seed
/// - N: The size of the radio buffer. Generally, this should be set to 256 to support the largest possible LoRa frames.
/// - D: The amount of downlinks that may be buffered. This is used to support Class C operation. See below for more.
///
/// Note that the const generics N and D are used to configure the size of the radio buffer and the number of downlinks
/// that may be buffered. The defaults are 256 and 1 respectively which should be fine for Class A devices. **For Class
/// C operation**, it is recommended to increase D to at least 2, if not 3. This is because during the RX1/RX2 windows
/// after a Class A transmit, it is possible to receive Class C downlinks (in additional to any RX1/RX2 responses!).
pub struct Device<R, T, G, const N: usize = 256, const D: usize = 1>
where
    R: radio::PhyRxTx + Timings,
    T: radio::Timer,
    G: RngCore,
{
    radio: R,
    /// Access to provided (pseudo)-random number generator.
    pub rng: G,
    timer: T,
    mac: Mac,
    radio_buffer: RadioBuffer<N>,
    downlink: Vec<Downlink, D>,
    #[cfg(feature = "class-c")]
    class_c: bool,
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(Debug)]
pub enum Error<R> {
    Radio(R),
    Mac(mac::Error),
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(Debug)]
pub enum SendResponse {
    DownlinkReceived(FcntDown),
    SessionExpired,
    NoAck,
    RxComplete,
    #[cfg(feature = "multicast")]
    Multicast(MulticastResponse),
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(Debug)]
pub enum JoinResponse {
    JoinSuccess,
    NoJoinAccept,
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(Debug)]
pub enum ListenResponse {
    SessionExpired,
    DownlinkReceived(FcntDown),
    #[cfg(feature = "multicast")]
    Multicast(MulticastResponse),
}

#[cfg(feature = "multicast")]
#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum MulticastResponse {
    NewSession { group_id: u8 },
    SessionExpired { group_id: u8 },
    DownlinkReceived { group_id: u8, fcnt: FcntDown },
}

impl<R> From<mac::Error> for Error<R> {
    fn from(e: mac::Error) -> Self {
        Error::Mac(e)
    }
}

impl<R, T, const N: usize> Device<R, T, rng::Prng, N>
where
    R: radio::PhyRxTx + Timings,
    T: radio::Timer,
{
    /// Create a new [`Device`] by providing your own random seed. Using this method, [`Device`] will internally
    /// use an algorithmic PRNG. Depending on your use case, this may or may not be faster than using your own
    /// hardware RNG.
    ///
    /// # ⚠️Warning⚠️
    ///
    /// This function must **always** be called with a new randomly generated seed! **Never** call this function more
    /// than once using the same seed. Generate the seed using a true random number generator. Using the same seed will
    /// leave you vulnerable to replay attacks.
    pub fn new_with_seed(region: region::Configuration, radio: R, timer: T, seed: u64) -> Self {
        Device::new_with_seed_and_session(region, radio, timer, seed, None)
    }

    /// Create a new [`Device`] by providing your own random seed. Also optionally provide your own [`Session`].
    /// Using this method, [`Device`] will internally use an algorithmic PRNG to generate random numbers. Depending on
    /// your use case, this may or may not be faster than using your own hardware RNG.
    ///
    /// # ⚠️Warning⚠️
    ///
    /// This function must **always** be called with a new randomly generated seed! **Never** call this function more
    /// than once using the same seed. Generate the seed using a true random number generator. Using the same seed will
    /// leave you vulnerable to replay attacks.
    pub fn new_with_seed_and_session(
        region: region::Configuration,
        radio: R,
        timer: T,
        seed: u64,
        session: Option<Session>,
    ) -> Self {
        let rng = rng::Prng::new(seed);
        Device::new_with_session(region, radio, timer, rng, session)
    }
}

impl<R, T, G, const N: usize, const D: usize> Device<R, T, G, N, D>
where
    R: radio::PhyRxTx + Timings,
    T: radio::Timer,
    G: RngCore,
{
    /// Create a new instance of [`Device`] with a RNG external to the LoRa chip. You must provide your own RNG
    /// implementing [`RngCore`].
    ///
    /// See also [`new_with_seed`](Device::new_with_seed) to let [`Device`] use a builtin PRNG by providing a random
    /// seed.
    pub fn new(region: region::Configuration, radio: R, timer: T, rng: G) -> Self {
        Device::new_with_session(region, radio, timer, rng, None)
    }

    /// Create a new [`Device`] and provide an optional [`Session`].
    pub fn new_with_session(
        region: region::Configuration,
        radio: R,
        timer: T,
        rng: G,
        session: Option<Session>,
    ) -> Self {
        let mut mac = Mac::new(region, R::MAX_RADIO_POWER, R::ANTENNA_GAIN);
        if let Some(session) = session {
            mac.set_session(session);
        }
        Self {
            radio,
            rng,
            mac,
            radio_buffer: RadioBuffer::new(),
            timer,
            downlink: Vec::new(),
            #[cfg(feature = "class-c")]
            class_c: false,
        }
    }

    /// Enables Class C behavior. Note that Class C downlinks are not possible until a confirmed
    /// uplink is sent to the LNS.
    #[cfg(feature = "class-c")]
    pub fn enable_class_c(&mut self) {
        self.class_c = true;
    }

    /// Sets the port range for frames sent to multicast groups. Warning: this exclusively handles
    /// these frames in the multicast context and, therefore, unicast frames in this range will not
    /// be handled. Defaults to `201..=205`.
    #[cfg(feature = "multicast")]
    pub fn set_multicast_port_range(&mut self, range: core::ops::RangeInclusive<u8>) {
        self.mac.multicast.set_range(range);
    }

    /// Sets the port for remote multicast setup messages used to derive multicast session keys.
    /// Warning: this exclusively handles these frames in the multicast layer and other application
    /// frames on this port will be ignored. Defaults to `200`.
    #[cfg(feature = "multicast")]
    pub fn set_multicast_remote_setup_port(&mut self, port: u8) {
        self.mac.multicast.set_remote_setup_port(port);
    }

    #[cfg(feature = "multicast")]
    /// Set the McKEKey for multicast session key derivation by providing a McRootKey.
    pub fn set_multicast_ke_key(&mut self, mc_root_key: McRootKey) {
        let crypto = DefaultFactory;
        let key = lorawan::keys::McKEKey::derive_from(&crypto, &mc_root_key);
        self.mac.multicast.mc_k_e_key = Some(key);
    }

    #[cfg(feature = "multicast")]
    /// In LoRaWAN 1.0.x, st the McKEKey for multicast session key derivation by providing an
    /// GenAppKey. The McRootKey is derived from this using `McRootKey = aes128_encrypt(GenAppKey, 0x00 | pad16) `
    /// and then the McKEKey is derived from the McRootKey.
    pub fn set_multicast_ke_key_from_gen_app_key(&mut self, key: GenAppKey) {
        let crypto = DefaultFactory;
        let mc_root_key = McRootKey::derive_from_gen_app_key(&crypto, &key);
        self.set_multicast_ke_key(mc_root_key);
    }

    #[cfg(feature = "multicast")]
    /// In LoRaWAN 1.1.x, st the McKEKey for multicast session key derivation by providing an
    /// GenAppKey. The McRootKey is derived from this using `McRootKey = aes128_encrypt(AppKey, 0x20 | pad16) `
    /// and then the McKEKey is derived from the McRootKey.
    pub fn set_multicast_ke_key_from_app_key(&mut self, key: AppKey) {
        let crypto = DefaultFactory;
        let mc_root_key = McRootKey::derive_from_app_key(&crypto, &key);
        self.set_multicast_ke_key(mc_root_key);
    }

    /// Sets a multicast session for this device for a specific group.
    #[cfg(feature = "multicast")]
    pub fn set_multicast_session(&mut self, group: McGroup, session: multicast::Session) {
        let index = match group {
            McGroup::_0 => 0,
            McGroup::_1 => 1,
            McGroup::_2 => 2,
            McGroup::_3 => 3,
        };
        self.mac.multicast.sessions[index] = Some(session);
    }

    /// Disables Class C behavior. Note that an uplink must be set for the radio to disable
    /// Class C listen.
    #[cfg(feature = "class-c")]
    pub fn disable_class_c(&mut self) {
        self.class_c = false;
    }

    pub fn get_session(&mut self) -> Option<&Session> {
        self.mac.get_session()
    }

    pub fn get_region(&mut self) -> &region::Configuration {
        &self.mac.region
    }

    pub fn get_radio(&mut self) -> &R {
        &self.radio
    }

    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.radio
    }

    /// Retrieve the current data rate being used by this device.
    pub fn get_datarate(&mut self) -> DR {
        self.mac.configuration.data_rate
    }

    /// Set the data rate being used by this device. This overrides the region default.
    pub fn set_datarate(&mut self, datarate: DR) {
        self.mac.configuration.data_rate = datarate;
    }

    /// Join the LoRaWAN network asynchronously. The returned future completes when
    /// the LoRaWAN network has been joined successfully, or an error has occurred.
    ///
    /// Repeatedly calling join using OTAA will result in a new LoRaWAN session to be created.
    ///
    /// Note that for a Class C enabled device, you must repeatedly send *confirmed* uplink until
    /// LoRaWAN Network Server (LNS) confirmation after joining.
    pub async fn join(&mut self, join_mode: &JoinMode) -> Result<JoinResponse, Error<R::PhyError>> {
        match join_mode {
            JoinMode::OTAA { deveui, appeui, appkey } => {
                let (tx_config, _) = self.mac.join_otaa::<G, N>(
                    &mut self.rng,
                    NetworkCredentials::new(*appeui, *deveui, *appkey),
                    &mut self.radio_buffer,
                );

                // Transmit the join payload
                let ms = self
                    .radio
                    .tx(tx_config, self.radio_buffer.as_ref_for_read())
                    .await
                    .map_err(Error::Radio)?;

                // Receive join response within RX window
                Ok(self.rx_downlink(&Frame::Join, ms).await?.into())
            }
            JoinMode::ABP { nwkskey, appskey, devaddr } => {
                self.mac.join_abp(*nwkskey, *appskey, *devaddr);
                Ok(JoinResponse::JoinSuccess)
            }
        }
    }

    /// Send data on a given port with the expected confirmation. If downlink data is provided, the
    /// data is copied into the provided byte slice.
    ///
    /// The returned future completes when the data have been sent successfully and downlink data,
    /// if any, is available by calling take_downlink. Response::DownlinkReceived indicates a
    /// downlink is available.
    ///
    /// In Class C mode, it is possible to get one or more downlinks and `Reponse::DownlinkReceived`
    /// maybe not even be indicated. It is recommended to call `take_downlink` after `send` until
    /// it returns `None`.
    pub async fn send(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) -> Result<SendResponse, Error<R::PhyError>> {
        // Prepare transmission buffer
        let (tx_config, _fcnt_up) = self.mac.send::<G, N>(
            &mut self.rng,
            &mut self.radio_buffer,
            &SendData { data, fport, confirmed },
        )?;
        // Transmit our data packet
        let ms = self
            .radio
            .tx(tx_config, self.radio_buffer.as_ref_for_read())
            .await
            .map_err(Error::Radio)?;

        // Wait for received data within window
        loop {
            let r = self.rx_downlink(&Frame::Data, ms).await?;
            println!("rx_downlink result: {:?}", r);
            match r {
                mac::Response::UplinkReady => {
                    continue;
                }
                r => {
                    return Ok(r.into());
                }
            }
        }
    }

    /// Take the downlink data from the device. This is typically called after a
    /// `Response::DownlinkReceived` is returned from `send`. This call consumes the downlink
    /// data. If no downlink data is available, `None` is returned.
    pub fn take_downlink(&mut self) -> Option<Downlink> {
        self.downlink.pop()
    }

    async fn window_complete(&mut self) -> Result<(), Error<R::PhyError>> {
        #[cfg(feature = "class-c")]
        if self.class_c {
            let rf_config = self.mac.get_rxc_config();
            return self.radio.setup_rx(rf_config).await.map_err(Error::Radio);
        }

        self.radio.low_power().await.map_err(Error::Radio)
    }

    #[cfg(not(feature = "class-c"))]
    async fn between_windows(
        &mut self,
        duration: u32,
    ) -> Result<Option<mac::Response>, Error<R::PhyError>> {
        self.radio.low_power().await.map_err(Error::Radio)?;
        self.timer.at(duration.into()).await;
        Ok(None)
    }

    #[cfg(feature = "class-c")]
    async fn between_windows(
        &mut self,
        duration: u32,
    ) -> Result<Option<mac::Response>, Error<R::PhyError>> {
        use self::radio::RxQuality;
        use futures::{future::select, future::Either, pin_mut};

        if !self.class_c {
            self.radio.low_power().await.map_err(Error::Radio)?;
            self.timer.at(duration.into()).await;
            return Ok(None);
        }

        #[allow(unused)]
        enum RxcWindowResponse<F: futures::Future<Output = ()> + Sized + Unpin> {
            Rx(usize, RxQuality, F),
            Timeout(u32),
        }

        /// RXC window listen until timeout
        async fn rxc_listen_until_timeout<F, R, const N: usize>(
            radio: &mut R,
            rx_buf: &mut RadioBuffer<N>,
            window_duration: u32,
            timeout_fut: F,
        ) -> RxcWindowResponse<F>
        where
            F: futures::Future<Output = ()> + Sized + Unpin,
            R: radio::PhyRxTx + Timings,
        {
            let rx_fut = radio.rx_continuous(rx_buf.as_mut());
            pin_mut!(rx_fut);
            // Wait until either a RF frame is received or the timeout future fires
            match select(rx_fut, timeout_fut).await {
                Either::Left((r, timeout_fut)) => match r {
                    Ok((sz, q)) => RxcWindowResponse::Rx(sz, q, timeout_fut),
                    // Ignore errors or timeouts and wait until the RX2 window is ready.
                    // Setting timeout to 0 ensures that `window_duration != rx2_start_delay`
                    _ => {
                        timeout_fut.await;
                        RxcWindowResponse::Timeout(0)
                    }
                },
                // Timeout! Prepare for the next window.
                Either::Right(_) => RxcWindowResponse::Timeout(window_duration),
            }
        }

        // Class C listen while waiting for the window
        let rx_config = self.mac.get_rxc_config();
        debug!("Configuring RXC window with config {}.", rx_config);
        self.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
        let mut response = None;
        let timeout_fut = self.timer.at(duration.into());
        pin_mut!(timeout_fut);
        let mut maybe_timeout_fut = Some(timeout_fut);

        // Keep processing RF frames until the timeout fires
        while let Some(timeout_fut) = maybe_timeout_fut.take() {
            match rxc_listen_until_timeout(
                &mut self.radio,
                &mut self.radio_buffer,
                duration,
                timeout_fut,
            )
            .await
            {
                RxcWindowResponse::Rx(sz, q, timeout_fut) => {
                    debug!("RXC window received {} bytes.", sz);
                    self.radio_buffer.set_pos(sz);
                    let mac_response = self.mac.handle_rxc::<N, D>(
                        &mut self.radio_buffer,
                        &mut self.downlink,
                        q.snr(),
                    )?;
                    match Self::handle_mac_response(
                        &mut self.radio_buffer,
                        &mut self.mac,
                        &mut self.radio,
                        &mut self.rng,
                        mac_response,
                        Some(rx_config),
                    )
                    .await?
                    {
                        None => {
                            debug!("RXC frame was invalid.");
                        }
                        Some(r) => {
                            debug!("Valid RXC frame received.");
                            // avoid overwriting new multicast session response
                            #[cfg(feature = "multicast")]
                            if let Some(mac::Response::Multicast(
                                multicast::Response::NewSession { .. },
                            )) = response
                            {
                                continue;
                            }
                            response = Some(r);
                        }
                    }
                    maybe_timeout_fut = Some(timeout_fut);
                }
                RxcWindowResponse::Timeout(_) => return Ok(response),
            }
        }
        Ok(response)
    }

    /// Attempt to receive data within RX1 and RX2 windows. This function will populate the
    /// provided buffer with data if received.
    async fn rx_downlink(
        &mut self,
        frame: &Frame,
        window_delay: u32,
    ) -> Result<mac::Response, Error<R::PhyError>> {
        self.timer.reset();
        self.radio_buffer.clear();

        println!("rx_downlink: INIT!");

        let rx1_start_delay = self.mac.get_rx_delay(frame, &Window::_1) + window_delay
            - self.radio.get_rx_window_lead_time_ms();

        debug!("Starting RX1 in {} ms.", rx1_start_delay);
        // sleep or RXC
        let _ = self.between_windows(rx1_start_delay).await?;

        // RX1
        let rx_config =
            self.mac.get_rx_config(self.radio.get_rx_window_buffer(), frame, &Window::_1);
        debug!("Configuring RX1 window with config {}.", rx_config);
        self.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;

        println!("rx_downlink: RX1!");
        if let Some(response) = self.rx_listen().await? {
            println!("RX1 received {:?}", response);
            return Ok(response);
        }

        let rx2_start_delay = self.mac.get_rx_delay(frame, &Window::_2) + window_delay
            - self.radio.get_rx_window_lead_time_ms();
        debug!("RX1 did not receive anything. Awaiting RX2 for {} ms.", rx2_start_delay);
        // sleep or RXC
        let _ = self.between_windows(rx2_start_delay).await?;

        // RX2
        let rx_config =
            self.mac.get_rx_config(self.radio.get_rx_window_buffer(), frame, &Window::_2);
        debug!("Configuring RX2 window with config {}.", rx_config);
        self.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;

        if let Some(response) = self.rx_listen().await? {
            debug!("RX2 received {}", response);
            return Ok(response);
        }
        debug!("RX2 did not receive anything.");
        Ok(self.mac.rx2_complete())
    }

    /// Helper function to handle MAC responses and perform common actions
    #[allow(unused_variables)]
    async fn handle_mac_response(
        radio_buffer: &mut RadioBuffer<N>,
        mac: &mut Mac,
        radio: &mut R,
        rng: &mut G,
        response: mac::Response,
        rx_config: Option<RxConfig>,
    ) -> Result<Option<mac::Response>, Error<R::PhyError>> {
        println!("Handle mac response!");
        radio_buffer.clear();
        match response {
            mac::Response::NoUpdate => {
                return Ok(None)
            }
            #[cfg(feature = "certification")]
            mac::Response::LinkCheckReq => {
                let _ = mac.add_uplink(lorawan::maccommandcreator::LinkCheckReqCreator::new());
                Ok(Some(mac.rx2_complete()))
            }
            #[cfg(feature = "certification")]
            mac::Response::UplinkReady => {
                let (tx_config, fcnt_up) =
                    mac.certification_setup_send::<G, N>(rng, radio_buffer)?;
                radio.tx(tx_config, radio_buffer.as_ref_for_read()).await.map_err(Error::Radio)?;
                // Signal device that Uplink is now ready and switch
                // to listen...
                Ok(Some(mac::Response::UplinkReady))
            }
            #[cfg(feature = "multicast")]
            mac::Response::Multicast(mut response) => {
                if response.is_transmit_request() {
                    let (tx_config, _fcnt_up) =
                        mac.multicast_setup_send::<G, N>(rng, radio_buffer)?;
                    radio
                        .tx(tx_config, radio_buffer.as_ref_for_read())
                        .await
                        .map_err(Error::Radio)?;
                    if let Some(rx_config) = rx_config {
                        radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
                    }
                    // GroupSetupTransmitRequest needs to be transformed into a NewSession response
                    if let multicast::Response::GroupSetupTransmitRequest { group_id } = response {
                        response = multicast::Response::NewSession { group_id };
                    }
                }
                if response.is_for_async_mc_response() {
                    Ok(Some(mac::Response::Multicast(response)))
                } else {
                    Ok(None)
                }
            }
            r => Ok(Some(r)),
        }
    }

    async fn rx_listen(&mut self) -> Result<Option<mac::Response>, Error<R::PhyError>> {
        let response =
            match self.radio.rx_single(self.radio_buffer.as_mut()).await.map_err(Error::Radio)? {
                RxStatus::Rx(s, q) => {
                    self.radio_buffer.set_pos(s);
                    let mac_response = self.mac.handle_rx::<N, D>(
                        &mut self.radio_buffer,
                        &mut self.downlink,
                        q.snr(),
                    );
                    Self::handle_mac_response(
                        &mut self.radio_buffer,
                        &mut self.mac,
                        &mut self.radio,
                        &mut self.rng,
                        mac_response,
                        None,
                    )
                    .await?
                }
                RxStatus::RxTimeout => None,
            };
        self.window_complete().await?;
        Ok(response)
    }

    /// When not involved in sending and RX1/RX2 windows, a class C configured device will be
    /// listening to RXC frames. The caller is expected to be awaiting this message at all times.
    #[cfg(feature = "class-c")]
    pub async fn rxc_listen(&mut self) -> Result<ListenResponse, Error<R::PhyError>> {
        let rx_config = self.mac.get_rxc_config();
        loop {
            let (sz, q) =
                self.radio.rx_continuous(self.radio_buffer.as_mut()).await.map_err(Error::Radio)?;
            self.radio_buffer.set_pos(sz);
            let mac_response =
                self.mac.handle_rxc::<N, D>(&mut self.radio_buffer, &mut self.downlink, q.snr())?;
            if let Some(response) = Self::handle_mac_response(
                &mut self.radio_buffer,
                &mut self.mac,
                &mut self.radio,
                &mut self.rng,
                mac_response,
                Some(rx_config),
            )
            .await?
            {
                return Ok(response.into());
            }
        }
    }
}

/// Allows to fine-tune the beginning and end of the receive windows for a specific board and runtime.
pub trait Timings {
    /// How many milliseconds before the RX window should the SPI transaction start?
    /// This value needs to account for the time it takes to wake up the radio and start the SPI transaction, as
    /// well as any non-deterministic delays in the system.
    fn get_rx_window_lead_time_ms(&self) -> u32;

    /// Explicitly set the amount of milliseconds to listen before the window starts. By default, the pessimistic assumption
    /// of `Self::get_rx_window_lead_time_ms` will be used. If you override, be sure that: `Self::get_rx_window_buffer
    /// < Self::get_rx_window_lead_time_ms`.
    fn get_rx_window_buffer(&self) -> u32 {
        self.get_rx_window_lead_time_ms()
    }
}
