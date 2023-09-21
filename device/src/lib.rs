#![cfg_attr(not(test), no_std)]
#![cfg_attr(feature = "async", feature(async_fn_in_trait))]
#![allow(incomplete_features)]

use heapless::Vec;

pub mod radio;

mod mac;
pub use mac::types::*;
use mac::Mac;

pub mod region;
pub use region::Region;

#[cfg(test)]
mod test_util;

mod nb_device;
use core::marker::PhantomData;
use lorawan::{
    keys::{CryptoFactory, AES128},
    parser::{DecryptedDataPayload, DevAddr, EUI64},
};
use nb_device::Shared;
pub use nb_device::{no_session, session};

pub use rand_core::RngCore;

#[cfg(feature = "async")]
pub mod async_device;

type TimestampMs = u32;

pub struct Device<R, C, RNG, const N: usize>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
    RNG: RngCore,
{
    state: Option<State>,
    shared: Shared<R, RNG, N>,
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
pub enum Error<R> {
    Radio(radio::Error<R>),
    Session(session::Error),
    NoSession(no_session::Error),
}

impl<R> From<radio::Error<R>> for Error<R> {
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
        write!(f, "lorawan_device::Event::{event}")
    }
}

pub struct SendData<'a> {
    data: &'a [u8],
    fport: u8,
    confirmed: bool,
}

#[allow(clippy::large_enum_variant)]
pub enum State {
    NoSession(no_session::NoSession),
    Session(session::Session),
}

use core::default::Default;

impl State {
    fn new() -> Self {
        State::NoSession(no_session::NoSession::new())
    }

    fn new_abp(
        newskey: NewSKey,
        appskey: AppSKey,
        devaddr: DevAddr<[u8; 4]>,
        region: region::Configuration,
    ) -> Self {
        let session_keys = SessionKeys::new(newskey, appskey, devaddr);
        State::Session(session::Session::new(session_keys, region))
    }
}

pub trait Timings {
    fn get_rx_window_offset_ms(&self) -> i32;
    fn get_rx_window_duration_ms(&self) -> u32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum JoinMode {
    OTAA { deveui: DevEui, appeui: AppEui, appkey: AppKey },
    ABP { newskey: NewSKey, appskey: AppSKey, devaddr: DevAddr<[u8; 4]> },
}
macro_rules! lorawan_key {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(AES128);
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $type(AES128);

        impl From<[u8;16]> for $type {
            fn from(key: [u8; 16]) -> Self {
                $type(AES128(key))
            }
        }
        };
    }

lorawan_key!(
    pub struct AppKey(AES128);
);
lorawan_key!(
    pub struct NewSKey(AES128);
);
lorawan_key!(
    pub struct AppSKey(AES128);
);

macro_rules! lorawan_eui {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(EUI64<[u8; 8]>);
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $type(EUI64<[u8; 8]>);

        impl From<[u8;8]> for $type {
            fn from(key: [u8; 8]) -> Self {
                $type(EUI64::from(key))
            }
        }
        };
    }
lorawan_eui!(
    pub struct DevEui(EUI64<[u8; 8]>);
);
lorawan_eui!(
    pub struct AppEui(EUI64<[u8; 8]>);
);

#[allow(dead_code)]
impl<R, C, RNG, const N: usize> Device<R, C, RNG, N>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
    RNG: RngCore,
{
    pub fn new(
        region: region::Configuration,
        join_mode: JoinMode,
        radio: R,
        rng: RNG,
    ) -> Device<R, C, RNG, N> {
        let (shared, state) = match join_mode {
            JoinMode::OTAA { deveui, appeui, appkey } => (
                Shared::new(
                    radio,
                    Some(Credentials::new(appeui, deveui, appkey)),
                    region,
                    Mac::default(),
                    rng,
                ),
                State::new(),
            ),
            JoinMode::ABP { newskey, appskey, devaddr } => (
                Shared::new(radio, None, region.clone(), Mac::default(), rng),
                State::new_abp(newskey, appskey, devaddr, region),
            ),
        };

        Device { crypto: PhantomData, shared, state: Some(state) }
    }

    pub fn get_radio(&mut self) -> &mut R {
        let shared = self.get_shared();
        shared.get_mut_radio()
    }

    pub fn get_credentials(&mut self) -> &mut Option<Credentials> {
        let shared = self.get_shared();
        shared.get_mut_credentials()
    }

    fn get_shared(&mut self) -> &mut Shared<R, RNG, N> {
        &mut self.shared
    }

    pub fn get_datarate(&mut self) -> region::DR {
        self.get_shared().get_datarate()
    }

    pub fn set_datarate(&mut self, datarate: region::DR) {
        self.get_shared().set_datarate(datarate);
    }

    pub fn ready_to_send_data(&self) -> bool {
        matches!(&self.state.as_ref().unwrap(), State::Session(session::Session::Idle(_)))
    }

    pub fn send(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) -> Result<Response, Error<R::PhyError>> {
        self.handle_event(Event::SendDataRequest(SendData { data, fport, confirmed }))
    }

    pub fn get_fcnt_up(&self) -> Option<u32> {
        if let State::Session(session) = &self.state.as_ref().unwrap() {
            Some(session.get_mac().fcnt_up())
        } else {
            None
        }
    }

    pub fn get_session_keys(&self) -> Option<&SessionKeys> {
        if let State::Session(session) = &self.state.as_ref().unwrap() {
            Some(session.get_session_keys())
        } else {
            None
        }
    }

    pub fn take_data_downlink(&mut self) -> Option<DecryptedDataPayload<Vec<u8, 256>>> {
        self.get_shared().take_data_downlink()
    }

    pub fn handle_event(&mut self, event: Event<R>) -> Result<Response, Error<R::PhyError>> {
        let (new_state, result) = match self.state.take().unwrap() {
            State::NoSession(state) => state.handle_event::<R, C, RNG, N>(event, &mut self.shared),
            State::Session(state) => state.handle_event::<R, C, RNG, N>(event, &mut self.shared),
        };
        self.state.replace(new_state);
        result
    }
}

/// Trait used to mark types which can give out an exclusice reference to [`RngCore`].
/// This trait is an implementation detail and should not be implemented outside this crate.
#[doc(hidden)]
pub trait GetRng: private::Sealed {
    type RNG: RngCore;
    fn get_rng(&mut self) -> &mut Self::RNG;
}

mod private {
    /// Super trait used to mark traits with an exhaustive set of
    /// implementations
    pub trait Sealed {}
}
