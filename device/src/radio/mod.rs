mod types;
pub use types::*;

use super::TimestampMs;

#[derive(Debug)]
pub enum Event<'a, R>
where
    R: PhyRxTx,
{
    TxRequest(TxConfig, &'a [u8]),
    RxRequest(RfConfig),
    CancelRx,
    PhyEvent(R::PhyEvent),
}

#[derive(Debug)]
pub enum Response<R>
where
    R: PhyRxTx,
{
    Idle,
    Txing,
    Rxing,
    TxDone(TimestampMs),
    RxDone(RxQuality),
    PhyResponse(R::PhyResponse),
}

#[derive(Debug)]
pub enum Error<R>
where
    R: PhyRxTx,
{
    TxRequestDuringTx,
    TxRequestDuringRx,
    RxRequestDuringTx,
    RxRequestDuringRx,
    CancelRxWhileIdle,
    CancelRxDuringTx,
    PhyError(R::PhyError),
}

use core::fmt;

pub trait PhyRxTx {
    type PhyEvent: fmt::Debug;
    type PhyError: fmt::Debug;
    type PhyResponse: fmt::Debug;

    fn get_mut_radio(&mut self) -> &mut Self;

    // we require mutability so we may decrypt in place
    fn get_received_packet(&mut self) -> &mut [u8];
    fn handle_event(&mut self, event: Event<Self>) -> Result<Response<Self>, Error<Self>>
    where
        Self: core::marker::Sized;
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
