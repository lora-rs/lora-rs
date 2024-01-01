//! A non-blocking LoRaWAN device implementation which uses an explicitly defined state machine
//! for driving the protocol state against pin and timer events. Depends on a non-async radio
//! implementation.
use super::radio::RadioBuffer;
use super::*;
use crate::nb_device::radio::PhyRxTx;
use mac::{Mac, SendData};

pub(crate) mod state;

pub mod radio;
#[cfg(test)]
mod test;

type TimestampMs = u32;

pub struct Device<R, C, RNG, const N: usize, const D: usize = 1>
where
    R: PhyRxTx + Timings,
    C: CryptoFactory + Default,
    RNG: RngCore,
{
    state: State,
    shared: Shared<R, RNG, N, D>,
    crypto: PhantomData<C>,
}

impl<R, C, RNG, const N: usize, const D: usize> Device<R, C, RNG, N, D>
where
    R: PhyRxTx + Timings,
    C: CryptoFactory + Default,
    RNG: RngCore,
{
    pub fn new(region: region::Configuration, radio: R, rng: RNG) -> Device<R, C, RNG, N, D> {
        Device {
            crypto: PhantomData,
            state: State::default(),
            shared: Shared {
                radio,
                rng,
                tx_buffer: RadioBuffer::new(),
                mac: Mac::new(region, R::MAX_RADIO_POWER, R::ANTENNA_GAIN),
                downlink: Vec::new(),
            },
        }
    }

    pub fn join(&mut self, join_mode: JoinMode) -> Result<Response, Error<R>> {
        match join_mode {
            JoinMode::OTAA { deveui, appeui, appkey } => {
                self.handle_event(Event::Join(NetworkCredentials::new(appeui, deveui, appkey)))
            }
            JoinMode::ABP { devaddr, appskey, newskey } => {
                self.shared.mac.join_abp(newskey, appskey, devaddr);
                Ok(Response::JoinSuccess)
            }
        }
    }

    pub fn get_radio(&mut self) -> &mut R {
        &mut self.shared.radio
    }

    pub fn get_datarate(&mut self) -> region::DR {
        self.shared.mac.configuration.data_rate
    }

    pub fn set_datarate(&mut self, datarate: region::DR) {
        self.shared.mac.configuration.data_rate = datarate
    }

    pub fn ready_to_send_data(&self) -> bool {
        matches!(&self.state, State::Idle(_)) && self.shared.mac.is_joined()
    }

    pub fn send(&mut self, data: &[u8], fport: u8, confirmed: bool) -> Result<Response, Error<R>> {
        self.handle_event(Event::SendDataRequest(SendData { data, fport, confirmed }))
    }

    pub fn get_fcnt_up(&self) -> Option<u32> {
        self.shared.mac.get_fcnt_up()
    }

    pub fn get_session(&self) -> Option<&mac::Session> {
        self.shared.mac.get_session()
    }

    pub fn set_session(&mut self, s: mac::Session) {
        self.shared.mac.set_session(s)
    }

    pub fn get_session_keys(&self) -> Option<mac::SessionKeys> {
        self.shared.mac.get_session_keys()
    }

    pub fn take_downlink(&mut self) -> Option<Downlink> {
        self.shared.downlink.pop()
    }

    pub fn handle_event(&mut self, event: Event<R>) -> Result<Response, Error<R>> {
        let (new_state, result) = self.state.handle_event::<R, C, RNG, N, D>(
            &mut self.shared.mac,
            &mut self.shared.radio,
            &mut self.shared.rng,
            &mut self.shared.tx_buffer,
            &mut self.shared.downlink,
            event,
        );
        self.state = new_state;
        result
    }
}

pub(crate) struct Shared<R: PhyRxTx + Timings, RNG: RngCore, const N: usize, const D: usize> {
    pub(crate) radio: R,
    pub(crate) rng: RNG,
    pub(crate) tx_buffer: RadioBuffer<N>,
    pub(crate) mac: Mac,
    pub(crate) downlink: Vec<Downlink, D>,
}

#[derive(Debug)]
pub enum Response {
    NoUpdate,
    TimeoutRequest(TimestampMs),
    JoinRequestSending,
    JoinSuccess,
    NoJoinAccept,
    UplinkSending(mac::FcntUp),
    DownlinkReceived(mac::FcntDown),
    NoAck,
    ReadyToSend,
    SessionExpired,
    RxComplete,
}

#[derive(Debug)]
pub enum Error<R: PhyRxTx> {
    Radio(R::PhyError),
    State(state::Error),
    Mac(mac::Error),
}

impl<R: PhyRxTx> From<mac::Error> for Error<R> {
    fn from(mac_error: mac::Error) -> Error<R> {
        Error::Mac(mac_error)
    }
}

pub enum Event<'a, R>
where
    R: PhyRxTx,
{
    Join(NetworkCredentials),
    SendDataRequest(SendData<'a>),
    RadioEvent(radio::Event<'a, R>),
    TimeoutFired,
}

impl<'a, R> core::fmt::Debug for Event<'a, R>
where
    R: PhyRxTx,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let event = match self {
            Event::Join(_) => "Join",
            Event::SendDataRequest(_) => "SendDataRequest",
            Event::RadioEvent(_) => "RadioEvent",
            Event::TimeoutFired => "TimeoutFired",
        };
        write!(f, "lorawan_device::Event::{event}")
    }
}
