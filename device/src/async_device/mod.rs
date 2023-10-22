//! Asynchronous Device using Rust async-await for driving the state machine,
//! and allowing asynchronous radio implementations. Requires the `async` feature and `nightly`.
use super::mac::Mac;

use super::mac::{self, Frame, Window};
pub use super::{
    mac::{NetworkCredentials, SendData, Session},
    region::{self, Region},
    Downlink, JoinMode, Timings,
};
use core::marker::PhantomData;
use futures::{future::select, future::Either, pin_mut};
use lorawan::{self, keys::CryptoFactory};

pub use crate::region::DR;
use crate::{
    radio::RadioBuffer,
    rng::{GetRng, NoneT, OptionalRng, Phy, RngCore},
};
#[cfg(feature = "external-lora-phy")]
/// provide the radio through the external lora-phy crate
pub mod lora_radio;
pub mod radio;

#[cfg(test)]
mod test;

use crate::radio::RxQuality;
use core::cmp::min;

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
    phy: Phy<R, G>,
    timer: T,
    mac: Mac,
    radio_buffer: RadioBuffer<N>,
    downlink: Option<Downlink>,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub enum Error<R> {
    Radio(R),
    Mac(mac::Error),
}

#[derive(Debug)]
pub enum SendResponse {
    DownlinkReceived(mac::FcntDown),
    SessionExpired,
    NoAck,
    RxComplete,
}

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
        session: Option<Session>,
    ) -> Self {
        let mut mac = Mac::new(region);
        if let Some(session) = session {
            mac.set_session(session);
        }
        Self {
            crypto: PhantomData,
            phy: Phy::new(radio, rng),
            mac,
            radio_buffer: RadioBuffer::new(),
            timer,
            downlink: None,
        }
    }

    pub fn get_session(&mut self) -> Option<&Session> {
        self.mac.get_session()
    }

    pub fn get_region(&mut self) -> &region::Configuration {
        &self.mac.region
    }

    pub fn get_radio(&mut self) -> &R {
        &self.phy.radio
    }

    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.phy.radio
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
    pub async fn join(&mut self, join_mode: &JoinMode) -> Result<JoinResponse, Error<R::PhyError>> {
        match join_mode {
            JoinMode::OTAA { deveui, appeui, appkey } => {
                let (tx_config, _) = self.mac.join_otaa::<C, Phy<R, G>, N>(
                    &mut self.phy,
                    NetworkCredentials::new(*appeui, *deveui, *appkey),
                    &mut self.radio_buffer,
                );

                // Transmit the join payload
                let ms = self
                    .phy
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
    pub async fn send(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) -> Result<SendResponse, Error<R::PhyError>> {
        // Prepare transmission buffer
        let (tx_config, _fcnt_up) = self.mac.send::<C, Phy<R, G>, N>(
            &mut self.phy,
            &mut self.radio_buffer,
            &SendData { data, fport, confirmed },
        )?;
        // Transmit our data packet
        let ms = self
            .phy
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
        self.downlink.take()
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
            + self.phy.radio.get_rx_window_offset_ms()) as u32;

        let rx1_end_delay = rx1_start_delay + self.phy.radio.get_rx_window_duration_ms();

        let rx2_start_delay = (self.mac.get_rx_delay(frame, &Window::_2) as i32
            + window_delay as i32
            + self.phy.radio.get_rx_window_offset_ms()) as u32;

        self.radio_buffer.clear();
        // Wait until RX1 window opens
        self.timer.at(rx1_start_delay.into()).await;

        let window_duration = {
            // Prepare for RX using correct configuration
            let rx_config =
                self.mac.region.get_rx_config(self.mac.configuration.data_rate, frame, &Window::_1);
            // Cap window duration so RX2 can start on time
            let mut window_duration = min(rx1_end_delay, rx2_start_delay);

            // Pass the full radio buffer slice to RX
            self.phy.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
            let timeout_fut = self.timer.at(window_duration.into());
            pin_mut!(timeout_fut);

            let mut maybe_timeout_fut = Some(timeout_fut);
            while let Some(timeout_fut) = maybe_timeout_fut.take() {
                match Self::rx_window(
                    &mut self.phy.radio,
                    &mut self.radio_buffer,
                    window_duration,
                    timeout_fut,
                )
                .await
                {
                    RxWindowResponse::Rx(sz, _, timeout_fut) => {
                        self.radio_buffer.set_pos(sz);
                        match self.mac.handle_rx::<C, N>(&mut self.radio_buffer, &mut self.downlink)
                        {
                            mac::Response::NoUpdate => {
                                self.radio_buffer.clear();
                                maybe_timeout_fut = Some(timeout_fut);
                            }
                            r => {
                                self.phy.radio.low_power().await.map_err(Error::Radio)?;
                                return Ok(r);
                            }
                        }
                    }
                    RxWindowResponse::Timeout(w) => {
                        window_duration = w;
                    }
                };
            }
            window_duration
        };

        if window_duration != rx2_start_delay {
            self.phy.radio.low_power().await.map_err(Error::Radio)?;
            self.timer.at(rx2_start_delay.into()).await;
        }

        // RX2
        // Prepare for RX using correct configuration
        let rx_config =
            self.mac.region.get_rx_config(self.mac.configuration.data_rate, frame, &Window::_2);
        let window_duration = self.phy.radio.get_rx_window_duration_ms();

        // Pass the full radio buffer slice to RX
        self.phy.radio.setup_rx(rx_config).await.map_err(Error::Radio)?;
        let timeout_fut = self.timer.delay_ms(window_duration.into());
        pin_mut!(timeout_fut);

        let mut maybe_timeout_fut = Some(timeout_fut);
        while let Some(timeout_fut) = maybe_timeout_fut.take() {
            match Self::rx_window(
                &mut self.phy.radio,
                &mut self.radio_buffer,
                window_duration,
                timeout_fut,
            )
            .await
            {
                RxWindowResponse::Rx(sz, _, timeout_fut) => {
                    self.radio_buffer.set_pos(sz);
                    match self.mac.handle_rx::<C, N>(&mut self.radio_buffer, &mut self.downlink) {
                        mac::Response::NoUpdate => {
                            self.radio_buffer.clear();
                            maybe_timeout_fut = Some(timeout_fut);
                        }
                        r => {
                            self.phy.radio.low_power().await.map_err(Error::Radio)?;
                            return Ok(r);
                        }
                    }
                }
                RxWindowResponse::Timeout(_) => {
                    self.phy.radio.low_power().await.map_err(Error::Radio)?;
                    return Ok(self.mac.rx2_complete());
                }
            };
        }
        panic!("Code should be unreachable.")
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
}
