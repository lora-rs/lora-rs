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

#[derive(Debug, Clone)]
pub struct Datarate {
    bandwidth: Bandwidth,
    spreading_factor: SpreadingFactor,
}

pub enum Frame {
    Join,
    Data,
}

pub enum Window {
    _1,
    _2,
}

impl Configuration {
    pub fn new(region: Region) -> Configuration {
        Configuration {
            state: State::new(region),
        }
    }
    pub(crate) fn create_tx_config(&mut self, random: u8, datarate: usize, frame: &Frame) -> TxConfig {
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

    pub(crate) fn get_rx_config(&mut self, datarate: usize, frame: &Frame, window: &Window) -> RfConfig {
        let datarate = self.get_rx_datarate(datarate, frame, window);
        RfConfig {
            frequency: self.get_rx_frequency(frame, window),
            bandwidth: datarate.bandwidth,
            spreading_factor: datarate.spreading_factor,
            coding_rate: self.get_coding_rate(),
        }
    }
}
macro_rules! from_region {
    ($r:tt) => {
    impl From<$r> for Configuration {
    fn from(region: $r) -> Configuration {
        Configuration {
            state: State::$r(region)
        }
    }
}
    }
}
from_region!(US915);
from_region!(CN470);
from_region!(EU868);

use super::state_machines::JoinAccept;
use lorawan_encoding::parser::DecryptedJoinAcceptPayload;

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

impl RegionHandler for Configuration {
    fn process_join_accept<T: core::convert::AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) -> JoinAccept {
        mut_region_dispatch!(self, process_join_accept, join_accept)
    }
    fn set_channel_mask(&mut self, channel_mask: ChannelMask) {
        mut_region_dispatch!(self, set_channel_mask, channel_mask)
    }
    fn set_subband(&mut self, subband: u8) {
        mut_region_dispatch!(self, set_subband, subband)
    }
    fn get_join_frequency(&mut self, random: u8) -> u32 {
        mut_region_dispatch!(self, get_join_frequency, random)
    }
    fn get_data_frequency(&mut self, random: u8) -> u32 {
        mut_region_dispatch!(self, get_data_frequency, random)
    }
    fn get_rx_delay(&self, frame: &Frame, window: &Window) -> u32 {
        region_dispatch!(self, get_rx_delay, frame, window)
    }
    fn get_rx_frequency(&self,frame: &Frame, window: &Window) -> u32 {
        region_dispatch!(self, get_rx_frequency, frame, window)
    }
    fn get_tx_datarate(&self, datarate: usize, frame: &Frame ) -> Datarate  {
        region_dispatch!(self, get_tx_datarate, datarate, frame)
    }
    fn get_rx_datarate(&self, datarate: usize, frame: &Frame, window: &Window ) -> Datarate {
        region_dispatch!(self, get_rx_datarate, datarate, frame, window)
    }
    fn get_dbm(&self) -> i8 {
        region_dispatch!(self, get_dbm)
    }
    fn get_coding_rate(&self) -> CodingRate {
        region_dispatch!(self, get_coding_rate)
    }
}

pub trait RegionHandler {
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
    fn get_rx_frequency(&self,frame: &Frame, window: &Window) -> u32;

    fn get_rx_delay(&self, frame: &Frame, window: &Window) -> u32 {
        match frame {
            Frame::Join => {
                match window {
                    Window::_1 => JOIN_ACCEPT_DELAY1,
                    Window::_2 => JOIN_ACCEPT_DELAY2,
                }
            }
            Frame::Data => {
                match window {
                    Window::_1 => RECEIVE_DELAY1,
                    Window::_2 => RECEIVE_DELAY2,
                }
            }
        }

    }

    fn get_tx_datarate(&self, datarate: usize, frame: &Frame ) -> Datarate;
    fn get_rx_datarate(&self, datarate: usize,  frame: &Frame, window: &Window ) -> Datarate;
    fn get_dbm(&self) -> i8 {
        DEFAULT_DBM
    }
    fn get_coding_rate(&self) -> CodingRate {
        DEFAULT_CODING_RATE
    }
}
