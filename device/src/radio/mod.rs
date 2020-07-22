use heapless::consts::*;
use heapless::Vec;

mod types;
pub use types::*;

use super::TimestampMs;

#[derive(Debug)]
pub enum Event<'a, R>
where
    R: PhyRxTx,
{
    TxRequest(TxConfig, &'a mut Vec<u8, U256>),
    RxRequest(RfConfig),
    CancelRx,
    PhyEvent(R::PhyEvent),
}

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

pub trait PhyRxTx {
    type PhyEvent;
    type PhyResponse;
    type PhyError;

    fn get_mut_radio(&mut self) -> &mut Self;

    // we require mutability so we may decrypt in place
    fn get_received_packet(&mut self) -> &mut Vec<u8, U256>;
    fn handle_event(&mut self, event: Event<Self>) -> Result<Response<Self>, Error<Self>>
    where
        Self: core::marker::Sized;
}
