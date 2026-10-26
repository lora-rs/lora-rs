#![deny(rust_2018_idioms)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! ## Feature flags
#![doc = document_features::document_features!()]
#![doc = include_str!("../README.md")]

// This must go FIRST so that all the other modules see its macros.
pub(crate) mod fmt;

use core::default::Default;
use heapless::Vec;

mod radio;

pub mod mac;
use mac::NetworkCredentials;

pub mod nvm;

pub mod region;
pub use region::Region;

#[cfg(test)]
mod test_util;

pub mod async_device;

pub mod nb_device;
use nb_device::state::State;

pub use lorawan::{
    keys::{AppEui, AppKey, AppSKey, Crypto, DevEui, NwkSKey},
    parser::DevAddr,
};

#[deprecated(since = "0.12.2", note = "Please use `NwkSKey` instead")]
pub use lorawan::keys::NwkSKey as NewSKey;

pub use rand_core::RngCore;
mod rng;
pub use rng::Prng;

/// Provides the application payload and FPort of a downlink message.
pub struct Downlink {
    pub data: Vec<u8, 256>,
    pub fport: u8,
}

#[cfg(feature = "defmt-03")]
impl defmt::Format for Downlink {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "Downlink {{ fport: {}, data: ", self.fport,);

        for byte in self.data.iter() {
            defmt::write!(f, "{:02x}", byte);
        }
        defmt::write!(f, " }}")
    }
}

/// Network time received in a `DeviceTimeAns`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct DeviceTime {
    /// Seconds since the GPS epoch (1980-01-06 00:00:00 UTC).
    pub seconds: u32,
    /// Sub-second part in 1/256 s.
    pub fraction: u8,
}

/// Application hooks for the physical device.
///
/// Implement this for the board/MCU type and pass it to
/// [`async_device::Device::new_with_handler`] or
/// [`nb_device::Device::new_with_handler`].
pub trait DeviceHandler {
    /// Battery level reported in `DevStatusAns`:
    /// - 0 - powered externally,
    /// - 1..=254 - battery level
    /// - 255 - battery status unknown
    /// Returns unknown status by default
    fn battery_level(&self) -> u8 {
        255
    }

    /// Called after the stack has handled a `DutResetReq` and reset its state,
    /// so the application can reboot the device (or MCU).
    /// The default does nothing.
    fn reboot(&mut self) {}

    /// Called when a `DeviceTimeAns` is received, with the network time.
    /// The application can use it to synchronize its RTC.
    /// The default does nothing.
    fn set_device_time(&mut self, _time: DeviceTime) {}
}

impl DeviceHandler for () {}

/// Allows to fine-tune the beginning and end of the receive windows for a specific board.
pub trait Timings {
    /// The offset in milliseconds from the beginning of the receive windows. For example, settings this to 100
    /// tell the LoRaWAN stack to begin configuring the receive window 100 ms before the window needs to start.
    fn get_rx_window_offset_ms(&self) -> i32;

    /// How long to leave the receive window open in milliseconds. For example, if offset was set to 100 and duration
    /// was set to 200, the window would be open 100 ms before and close 100 ms after the target time.
    fn get_rx_window_duration_ms(&self) -> u32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Join the network using either OTAA or ABP.
pub enum JoinMode {
    OTAA { deveui: DevEui, appeui: AppEui, appkey: AppKey },
    ABP { nwkskey: NwkSKey, appskey: AppSKey, devaddr: DevAddr },
}
