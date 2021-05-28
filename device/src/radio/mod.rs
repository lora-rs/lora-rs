mod types;
pub use types::*;

use super::TimestampMs;
use heapless::{ArrayLength, Vec};

#[derive(Debug)]
pub enum Event<'a, R>
where
    R: PhyRxTx,
{
    TxRequest(TxConfig, &'a mut R::PhyBuf),
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

pub trait PhyRxTxBuf {
    fn clear(&mut self);
    fn extend(&mut self, buf: &[u8]);
}

impl<N> PhyRxTxBuf for Vec<u8, N>
where
    N: ArrayLength<u8>,
{
    fn clear(&mut self) {
        self.clear();
    }

    fn extend(&mut self, buf: &[u8]) {
        self.extend_from_slice(buf).unwrap();
    }
}

pub trait PhyRxTx {
    type PhyBuf: AsRef<[u8]> + AsMut<[u8]> + Default + PhyRxTxBuf;
    type PhyEvent: fmt::Debug;
    type PhyError: fmt::Debug;
    type PhyResponse: fmt::Debug;

    fn get_mut_radio(&mut self) -> &mut Self;

    // we require mutability so we may decrypt in place
    fn get_received_packet(&mut self) -> &mut Self::PhyBuf;
    fn handle_event(&mut self, event: Event<Self>) -> Result<Response<Self>, Error<Self>>
    where
        Self: core::marker::Sized;
}
