#![allow(clippy::upper_case_acronyms)]
// generally, we allow upper_case_acronyms to make it match the LoRaWAN naming conventions better
use lorawan_encoding::maccommands::ChannelMask;

mod constants;
pub(crate) use crate::radio::*;
use constants::*;

mod cn470;
mod eu868;
mod us915;

pub use cn470::CN470;
pub use eu868::EU868;
pub use us915::US915;

pub struct Configuration {
    state: State,
}

// This datarate type is public to the device client
#[derive(Debug, Clone, Copy)]
pub enum DR {
    _0 = 0,
    _1 = 1,
    _2 = 2,
    _3 = 3,
    _4 = 4,
    _5 = 5,
    _6 = 6,
    _7 = 7,
    _8 = 8,
    _9 = 9,
    _10 = 10,
    _11 = 11,
    _12 = 12,
    _13 = 13,
    _14 = 14,
    _15 = 15,
}

#[derive(Debug, Clone)]
pub enum Region {
    US915,
    CN470,
    EU868,
}

enum State {
    US915(US915),
    CN470(CN470),
    EU868(EU868),
}

impl State {
    pub fn new(region: Region) -> State {
        match region {
            Region::US915 => State::US915(US915::new()),
            Region::CN470 => State::CN470(CN470::new()),
            Region::EU868 => State::EU868(EU868::new()),
        }
    }
}

// This datarate type is used internally for defining bandwidth/sf per region
#[derive(Debug, Clone)]
pub(crate) struct Datarate {
    bandwidth: Bandwidth,
    spreading_factor: SpreadingFactor,
}

pub(crate)  enum Frame {
    Join,
    Data,
}

pub(crate)  enum Window {
    _1,
    _2,
}

macro_rules! mut_region_dispatch {
  ($s:expr, $t:tt) => {
      match &mut $s.state {
        State::US915(state) => state.$t(),
        State::CN470(state) => state.$t(),
        State::EU868(state) => state.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &mut $s.state {
        State::US915(state) => state.$t($($arg)*),
        State::CN470(state) => state.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
    }
  };
}

macro_rules! region_dispatch {
  ($s:expr, $t:tt) => {
      match &$s.state {
        State::US915(state) => state.$t(),
        State::CN470(state) => state.$t(),
        State::EU868(state) => state.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &$s.state {
        State::US915(state) => state.$t($($arg)*),
        State::CN470(state) => state.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
    }
  };
}


impl Configuration {
    pub fn new(region: Region) -> Configuration {
        Configuration {
            state: State::new(region),
        }
    }
    pub(crate) fn create_tx_config(
        &mut self,
        random: u8,
        datarate: DR,
        frame: &Frame,
    ) -> TxConfig {
        let datarate = self.get_tx_datarate(datarate, frame);
        TxConfig {
            pw: self.get_dbm(),
            rf: RfConfig {
                frequency: match frame {
                    Frame::Data => self.get_data_frequency(random as u8),
                    Frame::Join => self.get_join_frequency(random as u8),
                },
                bandwidth: datarate.bandwidth,
                spreading_factor: datarate.spreading_factor,
                coding_rate: self.get_coding_rate(),
            },
        }
    }

    pub(crate) fn get_rx_config(
        &mut self,
        datarate: DR,
        frame: &Frame,
        window: &Window,
    ) -> RfConfig {
        let datarate = self.get_rx_datarate(datarate, frame, window);
        RfConfig {
            frequency: self.get_rx_frequency(frame, window),
            bandwidth: datarate.bandwidth,
            spreading_factor: datarate.spreading_factor,
            coding_rate: self.get_coding_rate(),
        }
    }

    pub(crate) fn process_join_accept<T: core::convert::AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) -> JoinAccept {
        mut_region_dispatch!(self, process_join_accept, join_accept)
    }

    pub(crate) fn set_channel_mask(&mut self, channel_mask: ChannelMask) {
        mut_region_dispatch!(self, set_channel_mask, channel_mask)
    }

    pub fn set_subband(&mut self, subband: u8) {
        mut_region_dispatch!(self, set_subband, subband)
    }

    pub(crate) fn get_join_frequency(&mut self, random: u8) -> u32 {
        mut_region_dispatch!(self, get_join_frequency, random)
    }
    pub(crate) fn get_data_frequency(&mut self, random: u8) -> u32 {
        mut_region_dispatch!(self, get_data_frequency, random)
    }
    pub(crate) fn get_rx_delay(&self, frame: &Frame, window: &Window) -> u32 {
        region_dispatch!(self, get_rx_delay, frame, window)
    }
    pub(crate) fn get_rx_frequency(&self, frame: &Frame, window: &Window) -> u32 {
        region_dispatch!(self, get_rx_frequency, frame, window)
    }
    pub(crate) fn get_default_datarate(&self) -> DR {
        region_dispatch!(self, get_default_datarate)
    }
    pub(crate) fn get_tx_datarate(&self, datarate: DR, frame: &Frame) -> Datarate {
        region_dispatch!(self, get_tx_datarate, datarate, frame)
    }
    pub(crate) fn get_rx_datarate(&self, datarate: DR, frame: &Frame, window: &Window) -> Datarate {
        region_dispatch!(self, get_rx_datarate, datarate, frame, window)
    }

    pub(crate) fn get_dbm(&self) -> i8 {
        region_dispatch!(self, get_dbm)
    }

    pub(crate) fn get_coding_rate(&self) -> CodingRate {
        region_dispatch!(self, get_coding_rate)
    }
}

macro_rules! from_region {
    ($r:tt) => {
        impl From<$r> for Configuration {
            fn from(region: $r) -> Configuration {
                Configuration {
                    state: State::$r(region),
                }
            }
        }
    };
}
from_region!(US915);
from_region!(CN470);
from_region!(EU868);

use super::state_machines::JoinAccept;
use lorawan_encoding::parser::DecryptedJoinAcceptPayload;


pub(crate) trait RegionHandler {
    fn process_join_accept<T: core::convert::AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) -> JoinAccept;
    fn set_channel_mask(&mut self, _channel_mask: ChannelMask) {
        // does not apply to every region
    }
    fn set_subband(&mut self, _subband: u8) {
        // does not apply to every region
    }

    fn get_join_frequency(&mut self, random: u8) -> u32;
    fn get_data_frequency(&mut self, random: u8) -> u32;
    fn get_rx_frequency(&self, frame: &Frame, window: &Window) -> u32;

    fn get_rx_delay(&self, frame: &Frame, window: &Window) -> u32 {
        match frame {
            Frame::Join => match window {
                Window::_1 => JOIN_ACCEPT_DELAY1,
                Window::_2 => JOIN_ACCEPT_DELAY2,
            },
            Frame::Data => match window {
                Window::_1 => RECEIVE_DELAY1,
                Window::_2 => RECEIVE_DELAY2,
            },
        }
    }
    fn get_default_datarate(&self) -> DR { DR::_0 }
    fn get_tx_datarate(&self, datarate: DR, frame: &Frame) -> Datarate;
    fn get_rx_datarate(&self, datarate: DR, frame: &Frame, window: &Window) -> Datarate;
    fn get_dbm(&self) -> i8 {
        DEFAULT_DBM
    }
    fn get_coding_rate(&self) -> CodingRate {
        DEFAULT_CODING_RATE
    }
}
