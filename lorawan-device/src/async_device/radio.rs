pub use crate::radio::{RfConfig, RxConfig, RxMode, RxQuality, TxConfig};

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct Error<E>(pub E);

impl<R> From<Error<R>> for super::Error<R> {
    fn from(radio_error: Error<R>) -> super::Error<R> {
        super::Error::Radio(radio_error.0)
    }
}

pub enum RxStatus {
    Rx(usize, RxQuality),
    RxTimeout,
}

/// An asynchronous timer that allows the state machine to await
/// between RX windows.
#[allow(async_fn_in_trait)]
pub trait Timer {
    fn reset(&mut self);

    /// Wait until millis milliseconds after reset has passed
    async fn at(&mut self, millis: u64);

    /// Delay for millis milliseconds
    async fn delay_ms(&mut self, millis: u64);
}

/// An asynchronous radio implementation that can transmit and receive data.
#[allow(async_fn_in_trait)]
pub trait PhyRxTx: Sized {
    type PhyError;

    /// Board-specific antenna gain and power loss in dBi.
    const ANTENNA_GAIN: i8 = 0;

    /// Maximum power (dBm) that the radio is able to output. When preparing instructions for radio,
    /// the value of maximum power will be used as an upper bound.
    const MAX_RADIO_POWER: u8;

    /// Transmit data buffer with the given transceiver configuration. The returned future
    /// should only complete once data have been transmitted.
    async fn tx(&mut self, config: TxConfig, buf: &[u8]) -> Result<u32, Self::PhyError>;

    /// Configures the radio to receive data. This future should not actually await the data itself.
    async fn setup_rx(&mut self, config: RxConfig) -> Result<(), Self::PhyError>;

    /// Configure a single receive window with its precomputed timing.
    ///
    /// The default preserves compatibility with radios that derive their
    /// timeout from [`RxConfig`]. Radios that support symbol-precise timeouts
    /// can override this method.
    async fn setup_rx_window(
        &mut self,
        config: RxConfig,
        _timing: super::RxWindowTiming,
    ) -> Result<(), Self::PhyError> {
        self.setup_rx(config).await
    }

    /// Receive data into the provided buffer with the given transceiver configuration. The returned
    /// future should only complete when RX data has been received. Furthermore, it should be
    /// possible to await the future again without settings up the receive config again.
    async fn rx_continuous(
        &mut self,
        rx_buf: &mut [u8],
    ) -> Result<(usize, RxQuality), Self::PhyError>;

    /// Receive data into the provided buffer with the given transceiver configuration. The returned
    /// future should complete when RX data has been received or when the timeout has expired.
    async fn rx_single(&mut self, buf: &mut [u8]) -> Result<RxStatus, Self::PhyError>;

    /// Puts the radio into a low-power mode.
    ///
    /// Used at the end of a receive cycle, before the potentially long idle
    /// until the next uplink. Radios should favor the lowest-current sleep here
    /// even when that means a slower, more expensive wake-up next time.
    async fn low_power(&mut self) -> Result<(), Self::PhyError> {
        Ok(())
    }

    /// Puts the radio into a low-power mode that keeps its configuration, for a
    /// short idle where a fast wake-up matters more than the sleep current.
    ///
    /// Called between the transmit and its receive windows, and between RX1 and
    /// RX2.
    async fn warm_sleep(&mut self) -> Result<(), Self::PhyError> {
        self.low_power().await
    }
}
