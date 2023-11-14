#![cfg_attr(not(test), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "async", feature(async_fn_in_trait))]
#![allow(incomplete_features)]
//#![feature(generic_const_exprs)]

//! ## Feature flags
#![doc = document_features::document_features!(feature_label = r#"<span class="stab portability"><code>{feature}</code></span>"#)]

use core::default::Default;
use heapless::Vec;

mod radio;

pub mod mac;
use mac::NetworkCredentials;

pub mod region;
pub use region::Region;

#[cfg(test)]
mod test_util;

pub mod nb_device;
use nb_device::state::State;

use core::marker::PhantomData;
use lorawan::{
    keys::{CryptoFactory, AES128},
    parser::{DevAddr, EUI64},
};

pub use rand_core::RngCore;
mod rng;
pub use rng::Prng;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub mod async_device;

#[derive(Debug)]
pub struct Downlink {
    pub data: Vec<u8, 256>,
    pub fport: u8,
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
