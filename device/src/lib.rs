#![no_std]

use heapless::consts::*;
use heapless::Vec;

pub mod radio;

mod mac;
use mac::Mac;

mod types;
pub use types::*;

pub mod region;
pub use region::Region;

mod state_machines;
use core::marker::PhantomData;
use lorawan_encoding::{keys::CryptoFactory, parser::DecryptedDataPayload};
use state_machines::Shared;
pub use state_machines::{no_session, session, JoinAccept};

type TimestampMs = u32;

pub struct Device<R, C>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
{
    state: State<R>,
    crypto: PhantomData<C>,
}

type FcntDown = u32;
type FcntUp = u32;

#[derive(Debug)]
pub enum Response {
    NoUpdate,
    TimeoutRequest(TimestampMs),
    JoinRequestSending,
    JoinSuccess,
    NoJoinAccept,
    UplinkSending(FcntUp),
    DownlinkReceived(FcntDown),
    NoAck,
    ReadyToSend,
    SessionExpired,
}

#[derive(Debug)]
pub enum Error<R: radio::PhyRxTx> {
    Radio(radio::Error<R>),
    Session(session::Error),
    NoSession(no_session::Error),
}

impl<R> From<radio::Error<R>> for Error<R>
where
    R: radio::PhyRxTx,
{
    fn from(radio_error: radio::Error<R>) -> Error<R> {
        Error::Radio(radio_error)
    }
}

pub enum Event<'a, R>
where
    R: radio::PhyRxTx,
{
    NewSessionRequest,
    SendDataRequest(SendData<'a>),
    RadioEvent(radio::Event<'a, R>),
    TimeoutFired,
}

impl<'a, R> core::fmt::Debug for Event<'a, R>
where
    R: radio::PhyRxTx,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let event = match self {
            Event::NewSessionRequest => "NewSessionRequest",
            Event::SendDataRequest(_) => "SendDataRequest",
            Event::RadioEvent(_) => "RadioEvent(?)",
            Event::TimeoutFired => "TimeoutFired",
        };
        write!(f, "lorawan_device::Event::{}", event)
    }
}

pub struct SendData<'a> {
    data: &'a [u8],
    fport: u8,
    confirmed: bool,
}

pub enum State<R>
where
    R: radio::PhyRxTx + Timings,
{
    NoSession(no_session::NoSession<R>),
    Session(session::Session<R>),
}

use core::default::Default;
impl<R> State<R>
where
    R: radio::PhyRxTx + Timings,
{
    fn new(shared: Shared<R>) -> Self {
        State::NoSession(no_session::NoSession::new(shared))
    }
}

pub trait Timings {
    fn get_rx_window_offset_ms(&self) -> i32;
    fn get_rx_window_duration_ms(&self) -> u32;
}

impl<R, C> Device<R, C>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
{
    pub fn new(
        region: region::Configuration,
        radio: R,
        deveui: [u8; 8],
        appeui: [u8; 8],
        appkey: [u8; 16],
        get_random: fn() -> u32,
    ) -> Device<R, C> {
        Device {
            crypto: PhantomData::default(),
            state: State::new(Shared::new(
                radio,
                Credentials::new(appeui, deveui, appkey),
                region,
                Mac::default(),
                get_random,
                Vec::new(),
            )),
        }
    }

    pub fn get_radio(&mut self) -> &mut R {
        let shared = self.get_shared();
        shared.get_mut_radio()
    }

    pub fn get_credentials(&mut self) -> &mut Credentials {
        let shared = self.get_shared();
        shared.get_mut_credentials()
    }

    fn get_shared(&mut self) -> &mut Shared<R> {
        match &mut self.state {
            State::NoSession(state) => state.get_mut_shared(),
            State::Session(state) => state.get_mut_shared(),
        }
    }

    pub fn ready_to_send_data(&self) -> bool {
        matches!(&self.state, State::Session(session::Session::Idle(_)))
    }

    pub fn send(
        self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) -> (Self, Result<Response, Error<R>>) {
        self.handle_event(Event::SendDataRequest(SendData {
            data,
            fport,
            confirmed,
        }))
    }

    pub fn get_fcnt_up(&self) -> Option<u32> {
        if let State::Session(session) = &self.state {
            Some(session.get_session_data().fcnt_up())
        } else {
            None
        }
    }

    pub fn get_session_keys(&self) -> Option<SessionKeys> {
        if let State::Session(session) = &self.state {
            Some(SessionKeys::copy_from_session_data(
                session.get_session_data(),
            ))
        } else {
            None
        }
    }

    pub fn take_data_downlink(&mut self) -> Option<DecryptedDataPayload<Vec<u8, U256>>> {
        self.get_shared().take_data_downlink()
    }

    pub fn take_join_accept(&mut self) -> Option<JoinAccept> {
        self.get_shared().take_join_accept()
    }

    pub fn handle_event(self, event: Event<R>) -> (Self, Result<Response, Error<R>>) {
        match self.state {
            State::NoSession(state) => state.handle_event(event),
            State::Session(state) => state.handle_event(event),
        }
    }
}
