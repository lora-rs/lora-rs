#![cfg_attr(not(test), no_std)]
#![cfg_attr(feature = "async", feature(async_fn_in_trait))]
#![allow(incomplete_features)]
use core::default::Default;
use heapless::Vec;

pub mod radio;
use radio::RadioBuffer;

mod mac;
use mac::NetworkCredentials;

pub mod region;
pub use region::Region;

#[cfg(test)]
mod test_util;

mod nb_device;
use nb_device::state::State;

use core::marker::PhantomData;
use lorawan::{
    keys::{CryptoFactory, AES128},
    parser::{DevAddr, EUI64},
};
use nb_device::Shared;

pub use rand_core::RngCore;
mod rng;

#[cfg(feature = "async")]
pub mod async_device;

type TimestampMs = u32;

pub struct Device<R, C, RNG, const N: usize>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
    RNG: RngCore,
{
    state: State,
    shared: Shared<R, RNG, N>,
    crypto: PhantomData<C>,
}

#[derive(Debug)]
pub struct Downlink {
    data: Vec<u8, 256>,
    fport: u8,
}

#[cfg(feature = "defmt")]
impl defmt::Format for Downlink {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "Downlink {{ fport: {}, data: ", self.fport,);

        for byte in self.data.iter() {
            defmt::write!(f, "{:02x}", byte);
        }
        defmt::write!(f, " }}")
    }
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
pub enum Error<R> {
    Radio(radio::Error<R>),
    Session(nb_device::state::Error),
    Mac(mac::Error),
}

impl<R> From<nb_device::state::Error> for Error<R> {
    fn from(error: nb_device::state::Error) -> Error<R> {
        Error::Session(error)
    }
}

impl<R> From<mac::Error> for Error<R> {
    fn from(mac_error: mac::Error) -> Error<R> {
        Error::Mac(mac_error)
    }
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
    Join(NetworkCredentials),
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
            Event::Join(_) => "Join",
            Event::SendDataRequest(_) => "SendDataRequest",
            Event::RadioEvent(_) => "RadioEvent",
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
    pub fn new(region: region::Configuration, radio: R, rng: RNG) -> Device<R, C, RNG, N> {
        Device {
            crypto: PhantomData,
            state: State::default(),
            shared: Shared {
                radio,
                rng,
                tx_buffer: RadioBuffer::new(),
                mac: mac::Mac::new(region),
                downlink: None,
            },
        }
    }

    pub fn join(&mut self, join_mode: JoinMode) -> Result<Response, Error<R::PhyError>> {
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
        self.shared.get_mut_radio()
    }

    pub fn get_datarate(&mut self) -> region::DR {
        self.shared.get_datarate()
    }

    pub fn set_datarate(&mut self, datarate: region::DR) {
        self.shared.set_datarate(datarate);
    }

    pub fn ready_to_send_data(&self) -> bool {
        matches!(&self.state, State::Idle(_)) && self.shared.mac.is_joined()
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
        self.shared.mac.get_fcnt_up()
    }

    pub fn get_session_keys(&self) -> Option<mac::SessionKeys> {
        self.shared.mac.get_session_keys()
    }

    pub fn take_downlink(&mut self) -> Option<Downlink> {
        self.shared.downlink.take()
    }

    pub fn handle_event(&mut self, event: Event<R>) -> Result<Response, Error<R::PhyError>> {
        let (new_state, result) = self.state.handle_event::<R, C, RNG, N>(
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
