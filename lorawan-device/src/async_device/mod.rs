//! LoRaWAN device which uses async-await for driving the protocol state against pin and timer events,
//! allowing for asynchronous radio implementations. Requires the `async` feature.
use super::mac::Mac;

use super::mac::{self, Frame, Window};
pub use super::{
    mac::{NetworkCredentials, SendData, Session},
    region::{self, Region},
    Downlink, JoinMode, Timings,
};
use core::marker::PhantomData;
use futures::{future::select, future::Either, pin_mut};
use heapless::Vec;
use lorawan::{self, keys::CryptoFactory};
use rand_core::RngCore;

pub use crate::region::DR;
use crate::{radio::RadioBuffer, rng};

pub mod radio;

#[cfg(feature = "embassy-time")]
mod embassy_time;
#[cfg(feature = "embassy-time")]
pub use embassy_time::EmbassyTimer;

#[cfg(test)]
mod test;

use crate::radio::RxQuality;
use core::cmp::min;

/// Type representing a LoRaWAN capable device.
///
/// A device is bound to the following types:
/// - R: An asynchronous radio implementation
/// - T: An asynchronous timer implementation
/// - C: A CryptoFactory implementation
/// - RNG: A random number generator implementation. An external RNG may be provided, or you may use a builtin PRNG by
/// providing a random seed
/// - N: The size of the radio buffer. Generally, this should be set to 256 to support the largest possible LoRa frames.
/// - D: The amount of downlinks that may be buffered. This is used to support Class C operation. See below for more.
///
/// Note that the const generics N and D are used to configure the size of the radio buffer and the number of downlinks
/// that may be buffered. The defaults are 256 and 1 respectively which should be fine for Class A devices. **For Class
/// C operation**, it is recommended to increase D to at least 2, if not 3. This is because during the RX1/RX2 windows
/// after a Class A transmit, it is possible to receive Class C downlinks (in additional to any RX1/RX2 responses!).
pub struct Device<R, C, T, G, const N: usize = 256, const D: usize = 1>
where
    R: radio::PhyRxTx + Timings,
    T: radio::Timer,
    C: CryptoFactory + Default,
    G: RngCore,
{
    crypto: PhantomData<C>,
    radio: R,
    rng: G,
    timer: T,
    mac: Mac,
    radio_buffer: RadioBuffer<N>,
    downlink: Vec<Downlink, D>,
    class_c: bool,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub enum Error<R> {
    Radio(R),
    Mac(mac::Error),
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub enum SendResponse {
    DownlinkReceived(mac::FcntDown),
    SessionExpired,
    NoAck,
    RxComplete,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub enum JoinResponse {
    JoinSuccess,
    NoJoinAccept,
}

impl<R> From<mac::Error> for Error<R> {
    fn from(e: mac::Error) -> Self {
        Error::Mac(e)
    }
}

// RX1
enum RxWindowResponse<F: futures::Future<Output = ()> + Sized + Unpin> {
    Rx(usize, RxQuality, F),
    Timeout(u32),
}

impl<R, C, T, const N: usize> Device<R, C, T, rng::Prng, N>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
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

impl<R, C, T, G, const N: usize, const D: usize> Device<R, C, T, G, N, D>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
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
            crypto: PhantomData,
            radio,
            rng,
            mac,
            radio_buffer: RadioBuffer::new(),
            timer,
            downlink: Vec::new(),
            class_c: false,
        }
    }

    /// Enables Class C behavior. Note that Class C downlinks are not possible until a confirmed
    /// uplink is sent to the LNS.

    pub fn enable_class_c(&mut self) {
        self.class_c = true;
    }

    /// Disables Class C behavior. Note that an uplink must be set for the radio to disable
    /// Class C listen.
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
                let (tx_config, _) = self.mac.join_otaa::<C, G, N>(
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
                self.timer.reset();
                Ok(self.rx_with_timeout(&Frame::Join, ms).await?.try_into()?)
            }
            JoinMode::ABP { newskey, appskey, devaddr } => {
                self.mac.join_abp(*newskey, *appskey, *devaddr);
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
        let (tx_config, _fcnt_up) = self.mac.send::<C, G, N>(
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
        self.timer.reset();
        Ok(self.rx_with_timeout(&Frame::Data, ms).await?.try_into()?)
    }

    /// Take the downlink data from the device. This is typically called after a
    /// `Response::DownlinkReceived` is returned from `send`. This call consumes the downlink
    /// data. If no downlink data is available, `None` is returned.
    pub fn take_downlink(&mut self) -> Option<Downlink> {
        self.downlink.pop()
    }

    async fn window_complete(&mut self) -> Result<(), Error<R::PhyError>> {
        if self.class_c {
            let rf_config = self.mac.get_rxc_config();
            self.radio.setup_rx(rf_config).await.map_err(Error::Radio)
        } else {
            self.radio.low_power().await.map_err(Error::Radio)
        }
    }

    async fn between_windows(
        &mut self,
        duration: u32,
    ) -> Result<Option<mac::Response>, Error<R::PhyError>> {
        if !self.class_c {
            self.radio.low_power().await.map_err(Error::Radio)?;
            self.timer.at(duration.into()).await;
            return Ok(None);
        }
        // Class C listen while waiting for the window
        let rf_config = self.mac.get_rxc_config();
        self.radio.setup_rx(rf_config).await.map_err(Error::Radio)?;
        let mut response = None;
        let timeout_fut = self.timer.at(duration.into());
        pin_mut!(timeout_fut);
        let mut maybe_timeout_fut = Some(timeout_fut);
        while let Some(timeout_fut) = maybe_timeout_fut.take() {
            match Self::rx_window(&mut self.radio, &mut self.radio_buffer, duration, timeout_fut)
                .await
            {
                RxWindowResponse::Rx(sz, _, timeout_fut) => {
                    self.radio_buffer.set_pos(sz);
                    match self
                        .mac
                        .handle_rxc::<C, N, D>(&mut self.radio_buffer, &mut self.downlink)?
                    {
                        mac::Response::NoUpdate => {
                            self.radio_buffer.clear();
                            maybe_timeout_fut = Some(timeout_fut);
                        }
                        r => {
                            self.radio_buffer.clear();
                            response = Some(r);
                        }
                    }
                }
                RxWindowResponse::Timeout(_) => return Ok(response),
            };
        }
        Ok(response)
    }

    /// Attempt to receive data within RX1 and RX2 windows. This function will populate the
    /// provided buffer with data if received. Will return a RxTimeout error if no RX within
    /// the windows.
    async fn rx_with_timeout(
        &mut self,
        frame: &Frame,
        window_delay: u32,
    ) -> Result<mac::Response, Error<R::PhyError>> {
        // The initial window configuration uses window 1 adjusted by window_delay and radio offset
        let rx1_start_delay = (self.mac.get_rx_delay(frame, &Window::_1) as i32
            + window_delay as i32
            + self.radio.get_rx_window_offset_ms()) as u32;

        let rx1_end_delay = rx1_start_delay + self.radio.get_rx_window_duration_ms();

        let rx2_start_delay = (self.mac.get_rx_delay(frame, &Window::_2) as i32
            + window_delay as i32
            + self.radio.get_rx_window_offset_ms()) as u32;
        self.radio_buffer.clear();
        let _ = self.between_windows(rx1_start_delay).await?;

        enum WindowResult {
            Rx(mac::Response),
            TimedOut(u32),
        }

        let window = {
            // Prepare for RX using correct configuration
            let rx_config = self.mac.get_rx_config(frame, &Window::_1);
            // Cap window duration so RX2 can start on time
            let mut window_duration = min(rx1_end_delay, rx2_start_delay);

            // Pass the full radio buffer slice to RX
            self.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
            let timeout_fut = self.timer.at(window_duration.into());
            pin_mut!(timeout_fut);

            let mut maybe_timeout_fut = Some(timeout_fut);
            let mut maybe_result = None;
            while let Some(timeout_fut) = maybe_timeout_fut.take() {
                match Self::rx_window(
                    &mut self.radio,
                    &mut self.radio_buffer,
                    window_duration,
                    timeout_fut,
                )
                .await
                {
                    RxWindowResponse::Rx(sz, _, timeout_fut) => {
                        self.radio_buffer.set_pos(sz);
                        match self
                            .mac
                            .handle_rx::<C, N, D>(&mut self.radio_buffer, &mut self.downlink)
                        {
                            mac::Response::NoUpdate => {
                                self.radio_buffer.clear();
                                maybe_timeout_fut = Some(timeout_fut);
                            }
                            r => {
                                maybe_result = Some(r);
                                break;
                            }
                        }
                    }
                    RxWindowResponse::Timeout(w) => {
                        window_duration = w;
                    }
                };
            }
            if let Some(r) = maybe_result {
                WindowResult::Rx(r)
            } else {
                WindowResult::TimedOut(window_duration)
            }
        };

        match window {
            // if we received a mac response above, we return early before rx
            WindowResult::Rx(r) => {
                self.window_complete().await?;
                return Ok(r);
            }
            // if we didn't receive a mac response, we continue to rx2
            WindowResult::TimedOut(window_duration) => {
                let _ = self.between_windows(window_duration).await?;
            }
        }

        let response = {
            // RX2
            // Prepare for RX using correct configuration
            let rx_config = self.mac.get_rx_config(frame, &Window::_2);
            let window_duration = self.radio.get_rx_window_duration_ms();

            // Pass the full radio buffer slice to RX
            self.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
            let timeout_fut = self.timer.delay_ms(window_duration.into());
            pin_mut!(timeout_fut);

            let mut maybe_timeout_fut = Some(timeout_fut);
            let mut maybe_result = None;
            while let Some(timeout_fut) = maybe_timeout_fut.take() {
                match Self::rx_window(
                    &mut self.radio,
                    &mut self.radio_buffer,
                    window_duration,
                    timeout_fut,
                )
                .await
                {
                    RxWindowResponse::Rx(sz, _, timeout_fut) => {
                        self.radio_buffer.set_pos(sz);
                        match self
                            .mac
                            .handle_rx::<C, N, D>(&mut self.radio_buffer, &mut self.downlink)
                        {
                            mac::Response::NoUpdate => {
                                self.radio_buffer.clear();
                                maybe_timeout_fut = Some(timeout_fut);
                            }
                            r => {
                                self.radio_buffer.clear();
                                maybe_result = Some(r);
                                break;
                            }
                        }
                    }
                    RxWindowResponse::Timeout(_) => (),
                };
            }
            if let Some(r) = maybe_result {
                r
            } else {
                self.mac.rx2_complete()
            }
        };
        self.window_complete().await?;
        Ok(response)
    }

    async fn rx_window<F>(
        radio: &mut R,
        rx_buf: &mut RadioBuffer<N>,
        window_duration: u32,
        timeout_fut: F,
    ) -> RxWindowResponse<F>
    where
        F: futures::Future<Output = ()> + Sized + Unpin,
    {
        let rx_fut = radio.rx(rx_buf.as_mut());
        pin_mut!(rx_fut);
        // Wait until either a RF frame is received or if we've reached window close
        match select(rx_fut, timeout_fut).await {
            // We've received an RF frame
            Either::Left((r, timeout_fut)) => match r {
                Ok((sz, q)) => RxWindowResponse::Rx(sz, q, timeout_fut),
                // Ignore errors or timeouts and wait until the RX2 window is ready.
                // Setting timeout to 0 ensures that `window_duration != rx2_start_delay`
                _ => {
                    timeout_fut.await;
                    RxWindowResponse::Timeout(0)
                }
            },
            // Timeout! Prepare for the next window.
            Either::Right(_) => RxWindowResponse::Timeout(window_duration),
        }
    }

    /// When not involved in sending and RX1/RX2 windows, a class C configured device will be
    /// listening to RXC frames. The caller is expected to be awaiting this message at all times.
    pub async fn rxc_listen(&mut self) -> Result<mac::Response, Error<R::PhyError>> {
        loop {
            let (sz, _rx_quality) =
                self.radio.rx(self.radio_buffer.as_mut()).await.map_err(Error::Radio)?;
            self.radio_buffer.set_pos(sz);
            match self.mac.handle_rxc::<C, N, D>(&mut self.radio_buffer, &mut self.downlink)? {
                mac::Response::NoUpdate => {
                    self.radio_buffer.clear();
                }
                r => {
                    self.radio_buffer.clear();
                    return Ok(r);
                }
            }
        }
    }
}
