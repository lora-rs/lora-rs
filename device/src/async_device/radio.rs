pub use crate::radio::{types::*, RfConfig, RxQuality, TxConfig};
use core::future::Future;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Error<R: PhyRxTx>(pub R::PhyError);

impl<R> From<Error<R>> for super::Error<R>
where
    R: PhyRxTx,
{
    fn from(radio_error: Error<R>) -> super::Error<R> {
        super::Error::Radio(radio_error.0)
    }
}

/// An asynchronous timer that allows the state machine to await
/// between RX windows.
pub trait Timer {
    type DelayFuture<'m>: Future<Output = ()> + 'm
    where
        Self: 'm;
    fn delay_ms<'m>(&'m mut self, millis: u64) -> Self::DelayFuture<'m>;
}

/// An asynchronous radio implementation that can transmit and receive data.
pub trait PhyRxTx: Sized {
    #[cfg(feature = "defmt")]
    type PhyError: defmt::Format;

    #[cfg(not(feature = "defmt"))]
    type PhyError;

    type TxFuture<'m>: Future<Output = Result<u32, Self::PhyError>> + 'm
    where
        Self: 'm;

    /// Transmit data buffer with the given tranciever configuration. The returned future
    /// should only complete once data have been transmitted.
    fn tx<'m>(&'m mut self, config: TxConfig, buf: &'m [u8]) -> Self::TxFuture<'m>;

    type RxFuture<'m>: Future<Output = Result<(usize, RxQuality), Self::PhyError>> + 'm
    where
        Self: 'm;
    /// Receive data into the provided buffer with the given tranciever configuration. The returned future
    /// should only complete when RX data have been received.
    fn rx<'m>(&'m mut self, config: RfConfig, rx_buf: &'m mut [u8]) -> Self::RxFuture<'m>;
}
