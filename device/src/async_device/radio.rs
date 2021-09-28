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
    type DelayFuture<'m>: Future<Output = ()> + 'm;
    fn delay_ms<'m>(&'m mut self, millis: u64) -> Self::DelayFuture<'m>;
}

/// An asynchronous radio implementation that can transmit and receive data.
pub trait PhyRxTx: Sized {
    #[cfg(feature = "defmt")]
    type PhyError: defmt::Format;

    #[cfg(not(feature = "defmt"))]
    type PhyError;

    type TxFuture<'m>: Future<Output = Result<u32, Self::PhyError>> + 'm;

    /// Transmit data buffer with the given tranciever configuration. The returned future
    /// should only complete once data have been transmitted.
    fn tx<'m>(&'m mut self, config: TxConfig, buf: &'m [u8]) -> Self::TxFuture<'m>;

    type RxFuture<'m>: Future<Output = Result<(usize, RxQuality), Self::PhyError>> + 'm;
    /// Receive data into the provided buffer with the given tranciever configuration. The returned future
    /// should only complete when RX data have been received.
    fn rx<'m>(&'m mut self, config: RfConfig, rx_buf: &'m mut [u8]) -> Self::RxFuture<'m>;
}

pub struct RadioBuffer<'a> {
    packet: &'a mut [u8],
    pos: usize,
}

impl<'a> RadioBuffer<'a> {
    pub fn new(packet: &'a mut [u8]) -> Self {
        Self { packet, pos: 0 }
    }

    pub fn clear(&mut self) {
        self.pos = 0;
    }

    pub fn as_raw_slice(&mut self) -> &mut [u8] {
        &mut self.packet
    }

    pub fn inc(&mut self, n: usize) {
        self.pos += core::cmp::min(n, self.packet.len());
    }

    pub fn extend_from_slice(&mut self, buf: &[u8]) -> Result<(), ()> {
        if self.pos + buf.len() < self.packet.len() {
            self.packet[self.pos..self.pos + buf.len()].copy_from_slice(buf);
            self.pos += buf.len();
            Ok(())
        } else {
            Err(())
        }
    }
}

impl AsMut<[u8]> for RadioBuffer<'_> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.packet[..self.pos]
    }
}

impl AsRef<[u8]> for RadioBuffer<'_> {
    fn as_ref(&self) -> &[u8] {
        &self.packet[..self.pos]
    }
}
