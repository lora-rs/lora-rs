#![allow(clippy::upper_case_acronyms)]
// generally, we allow upper_case_acronyms to make it match the LoRaWAN naming conventions better
use lorawan::{maccommands::ChannelMask, parser::CfList};

pub mod constants;
use crate::mac;
pub(crate) use crate::radio::*;
use constants::*;

mod as923;
mod au915;
mod cn470;
mod eu433;
mod eu868;
mod in865;
mod us915;

pub use as923::AS923;
pub use au915::AU915;
pub use cn470::CN470;
pub use eu433::EU433;
pub use eu868::EU868;
pub use in865::IN865;
pub use us915::US915;

pub struct Configuration {
    state: State,
    join_accept_delay1: u32,
    join_accept_delay2: u32,
    receive_delay1: u32,
    receive_delay2: u32,
}

// This datarate type is public to the device client
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    EU433,
    AU915,
    AS923_1,
    AS923_2,
    AS923_3,
    AS923_4,
    IN865,
}

enum State {
    US915(US915),
    CN470(CN470),
    EU868(EU868),
    EU433(EU433),
    AU915(AU915),
    AS923(AS923),
    IN865(IN865),
}

impl State {
    pub fn new(region: Region) -> State {
        match region {
            Region::US915 => State::US915(US915::new()),
            Region::CN470 => State::CN470(CN470::new()),
            Region::EU868 => State::EU868(EU868::new()),
            Region::EU433 => State::EU433(EU433::new()),
            Region::AU915 => State::AU915(AU915::new()),
            Region::AS923_1 => State::AS923(AS923::new(as923::AS923Subband::_1)),
            Region::AS923_2 => State::AS923(AS923::new(as923::AS923Subband::_2)),
            Region::AS923_3 => State::AS923(AS923::new(as923::AS923Subband::_3)),
            Region::AS923_4 => State::AS923(AS923::new(as923::AS923Subband::_4)),
            Region::IN865 => State::IN865(IN865::new()),
        }
    }
}

// This datarate type is used internally for defining bandwidth/sf per region
#[derive(Debug, Clone)]
pub(crate) struct Datarate {
    bandwidth: Bandwidth,
    spreading_factor: SpreadingFactor,
}

pub(crate) enum Frame {
    Join,
    Data,
}

pub(crate) enum Window {
    _1,
    _2,
}

macro_rules! mut_region_dispatch {
  ($s:expr, $t:tt) => {
      match &mut $s.state {
        State::US915(state) => state.$t(),
        State::CN470(state) => state.$t(),
        State::EU868(state) => state.$t(),
        State::EU433(state) => state.$t(),
        State::AU915(state) => state.$t(),
        State::AS923(state) => state.$t(),
        State::IN865(state) => state.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &mut $s.state {
        State::US915(state) => state.$t($($arg)*),
        State::CN470(state) => state.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
        State::EU433(state) => state.$t($($arg)*),
        State::AU915(state) => state.$t($($arg)*),
        State::AS923(state) => state.$t($($arg)*),
        State::IN865(state) => state.$t($($arg)*),
    }
  };
}

macro_rules! region_dispatch {
  ($s:expr, $t:tt) => {
      match &$s.state {
        State::US915(state) => state.$t(),
        State::CN470(state) => state.$t(),
        State::EU868(state) => state.$t(),
        State::EU433(state) => state.$t(),
        State::AU915(state) => state.$t(),
        State::AS923(state) => state.$t(),
        State::IN865(state) => state.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &$s.state {
        State::US915(state) => state.$t($($arg)*),
        State::CN470(state) => state.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
        State::EU433(state) => state.$t($($arg)*),
        State::AU915(state) => state.$t($($arg)*),
        State::AS923(state) => state.$t($($arg)*),
        State::IN865(state) => state.$t($($arg)*),
    }
  };
}

impl Configuration {
    pub fn new(region: Region) -> Configuration {
        Configuration::with_state(State::new(region))
    }

    fn with_state(state: State) -> Configuration {
        Configuration {
            state,
            receive_delay1: constants::RECEIVE_DELAY1,
            receive_delay2: constants::RECEIVE_DELAY2,
            join_accept_delay1: constants::JOIN_ACCEPT_DELAY1,
            join_accept_delay2: constants::JOIN_ACCEPT_DELAY2,
        }
    }

    // RECEIVE_DELAY2 is not configurable, LoRaWAN 1.0.3 Section 5.7: "The second reception slot opens one second after the first reception slot."
    pub fn set_receive_delay1(&mut self, delay: u32) {
        self.receive_delay1 = delay;
        self.receive_delay2 = self.receive_delay1 + 1000;
    }

    pub fn set_join_accept_delay1(&mut self, delay: u32) {
        self.join_accept_delay1 = delay;
    }

    pub fn set_join_accept_delay2(&mut self, delay: u32) {
        self.join_accept_delay2 = delay;
    }

    pub(crate) fn create_tx_config(&mut self, random: u8, datarate: DR, frame: &Frame) -> TxConfig {
        let (dr, frequency) = self.get_tx_dr_and_frequency(random, datarate, frame);
        TxConfig {
            pw: self.get_dbm(),
            rf: RfConfig {
                frequency,
                bandwidth: dr.bandwidth,
                spreading_factor: dr.spreading_factor,
                coding_rate: self.get_coding_rate(),
            },
        }
    }

    fn get_tx_dr_and_frequency(
        &mut self,
        random: u8,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        mut_region_dispatch!(self, get_tx_dr_and_frequency, random, datarate, frame)
    }

    pub(crate) fn get_rx_config(
        &mut self,
        datarate: DR,
        frame: &Frame,
        window: &Window,
    ) -> RfConfig {
        let dr = self.get_rx_datarate(datarate, frame, window);
        RfConfig {
            frequency: self.get_rx_frequency(frame, window),
            bandwidth: dr.bandwidth,
            spreading_factor: dr.spreading_factor,
            coding_rate: self.get_coding_rate(),
        }
    }

    pub(crate) fn process_join_accept<T: core::convert::AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) -> JoinAccept {
        self.set_receive_delay1(mac::del_to_delay_ms(join_accept.rx_delay()));
        mut_region_dispatch!(self, process_join_accept, join_accept)
    }

    pub(crate) fn set_channel_mask(&mut self, channel_mask: ChannelMask) {
        mut_region_dispatch!(self, set_channel_mask, channel_mask)
    }

    pub fn set_subband(&mut self, subband: u8) {
        mut_region_dispatch!(self, set_subband, subband)
    }
    pub(crate) fn get_rx_delay(&self, frame: &Frame, window: &Window) -> u32 {
        match frame {
            Frame::Join => match window {
                Window::_1 => self.join_accept_delay1,
                Window::_2 => self.join_accept_delay2,
            },
            Frame::Data => match window {
                Window::_1 => self.receive_delay1,
                Window::_2 => self.receive_delay2,
            },
        }
    }
    pub(crate) fn get_rx_frequency(&self, frame: &Frame, window: &Window) -> u32 {
        region_dispatch!(self, get_rx_frequency, frame, window)
    }
    pub(crate) fn get_default_datarate(&self) -> DR {
        region_dispatch!(self, get_default_datarate)
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
                Configuration::with_state(State::$r(region))
            }
        }
    };
}

from_region!(US915);
from_region!(CN470);
from_region!(EU868);
from_region!(EU433);
from_region!(AU915);
from_region!(AS923);
from_region!(IN865);

use super::state_machines::JoinAccept;
use lorawan::parser::DecryptedJoinAcceptPayload;

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
    fn get_default_datarate(&self) -> DR {
        DR::_0
    }

    fn get_tx_dr_and_frequency(
        &mut self,
        random: u8,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32);

    fn get_rx_frequency(&self, frame: &Frame, window: &Window) -> u32;
    fn get_rx_datarate(&self, datarate: DR, frame: &Frame, window: &Window) -> Datarate;
    fn get_dbm(&self) -> i8 {
        DEFAULT_DBM
    }
    fn get_coding_rate(&self) -> CodingRate {
        DEFAULT_CODING_RATE
    }
}
