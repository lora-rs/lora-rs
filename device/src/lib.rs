#![no_std]

use heapless::consts::*;
use heapless::Vec;

pub mod radio;

mod mac;
use mac::Mac;

mod types;
pub use types::*;

mod us915;
use us915::Configuration as RegionalConfiguration;

mod state_machines;
use state_machines::Shared;
pub use state_machines::{no_session, session};

type TimestampMs = u32;

pub struct Device<R: radio::PhyRxTx + Timings> {
    state: State<R>,
}

#[derive(Debug)]
pub enum Response {
    Idle,
    DataDown,   // packet received
    TxComplete, // packet sent
    TimeoutRequest(TimestampMs),
    SendingJoinRequest,
    WaitingForJoinAccept,
    Rxing,
    NewSession,
    SendingDataUp,
    WaitingForDataDown,
    NoAck,
    ReadyToSend,
}

pub enum Error<R: radio::PhyRxTx> {
    Radio(radio::Error<R>), // error: unhandled event
    Session(session::Error),
    NoSession(no_session::Error),
}

impl<R> From<radio::Error<R>> for Error<R>
where R: radio::PhyRxTx
{
    fn from(radio_error: radio::Error<R>) -> Error<R> {
        Error::Radio(radio_error)
    }
}

pub enum Event<'a, R>
where
    R: radio::PhyRxTx,
{
    NewSession,
    RadioEvent(radio::Event<'a, R>),
    Timeout,
    SendData(SendData<'a>),
}

impl<'a, R> core::fmt::Debug for Event<'a, R>
where
    R: radio::PhyRxTx,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let event = match self {
            Event::NewSession => "NewSession",
            Event::RadioEvent(_) => "RadioEvent(?)",
            Event::Timeout => "Timeout",
            Event::SendData(_) => "SendData",
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

trait CommonState<R: radio::PhyRxTx + Timings> {
    fn get_mut_shared(&mut self) -> &mut Shared<R>;
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
    fn get_rx_window_offset_ms(&mut self) -> i32;
    fn get_rx_window_duration_ms(&mut self) -> u32;
}

impl<R: radio::PhyRxTx + Timings> Device<R> {
    pub fn new(
        radio: R,
        deveui: [u8; 8],
        appeui: [u8; 8],
        appkey: [u8; 16],
        get_random: fn() -> u32,
    ) -> Device<R> {
        let mut region = RegionalConfiguration::new();
        region.set_subband(2);

        Device {
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

    pub fn send(
        self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) -> (Self, Result<Response, Error<R>>) {
        self.handle_event(Event::SendData(SendData {
            data,
            fport,
            confirmed,
        }))
    }

    pub fn handle_event(self, event: Event<R>) -> (Self, Result<Response, Error<R>>) {
        match self.state {
            State::NoSession(state) => state.handle_event(event),
            State::Session(state) => state.handle_event(event),
        }
        // self.state = new_state;
        // (self, response)
    }
}
