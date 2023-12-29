#![cfg_attr(not(test), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(async_fn_in_trait)]

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
pub use lorawan::{
    keys::{AppEui, AppKey, AppSKey, CryptoFactory, DevEui, NewSKey},
    parser::DevAddr,
    default_crypto,
};

pub use rand_core::RngCore;
mod rng;
pub use rng::Prng;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub mod async_device;

#[derive(Debug)]
/// Provides the application payload and FPort of a downlink message.
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
/// Join the network using either OTAA or ABP.
pub enum JoinMode {
    OTAA { deveui: DevEui, appeui: AppEui, appkey: AppKey },
    ABP { newskey: NewSKey, appskey: AppSKey, devaddr: DevAddr<[u8; 4]> },
}
