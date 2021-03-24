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

pub(crate) enum Frame {
    Join,
    Data,
}

impl Configuration {
    pub fn new(region: Region) -> Configuration {
        Configuration {
            state: State::new(region),
        }
    }

    pub(crate) fn create_tx_config(&mut self, random: u8, frame: Frame) -> TxConfig {
        TxConfig {
            pw: self.get_dbm(),
            rf: RfConfig {
                frequency: match frame {
                    Frame::Data => self.get_data_frequency(random as u8),
                    Frame::Join => self.get_join_frequency(random as u8),
                },
                bandwidth: self.get_bandwidth(),
                spreading_factor: self.get_spreading_factor(),
                coding_rate: self.get_coding_rate(),
            },
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
    fn get_join_accept_frequency1(&self) -> u32 {
        region_dispatch!(self, get_join_accept_frequency1)
    }
    fn get_rxwindow1_frequency(&self) -> u32 {
        region_dispatch!(self, get_rxwindow1_frequency)
    }
    fn get_join_accept_delay1(&self) -> u32 {
        region_dispatch!(self, get_join_accept_delay1)
    }
    fn get_join_accept_delay2(&self) -> u32 {
        region_dispatch!(self, get_join_accept_delay2)
    }
    fn get_receive_delay1(&self) -> u32 {
        region_dispatch!(self, get_receive_delay1)
    }
    fn get_receive_delay2(&self) -> u32 {
        region_dispatch!(self, get_receive_delay2)
    }
    fn get_bandwidth(&self) -> Bandwidth {
        region_dispatch!(self, get_bandwidth)
    }
    fn get_dbm(&self) -> i8 {
        region_dispatch!(self, get_dbm)
    }
    fn get_coding_rate(&self) -> CodingRate {
        region_dispatch!(self, get_coding_rate)
    }
    fn get_spreading_factor(&self) -> SpreadingFactor {
        region_dispatch!(self, get_spreading_factor)
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
    fn get_join_accept_frequency1(&self) -> u32;
    fn get_rxwindow1_frequency(&self) -> u32;

    fn get_join_accept_delay1(&self) -> u32 {
        JOIN_ACCEPT_DELAY1
    }
    fn get_join_accept_delay2(&self) -> u32 {
        JOIN_ACCEPT_DELAY2
    }

    fn get_receive_delay1(&self) -> u32 {
        RECEIVE_DELAY1
    }

    fn get_receive_delay2(&self) -> u32 {
        RECEIVE_DELAY2
    }

    fn get_bandwidth(&self) -> Bandwidth {
        DEFAULT_BANDWIDTH
    }
    fn get_dbm(&self) -> i8 {
        DEFAULT_DBM
    }
    fn get_coding_rate(&self) -> CodingRate {
        DEFAULT_CODING_RATE
    }
    fn get_spreading_factor(&self) -> SpreadingFactor {
        DEFAULT_SPREADING_FACTOR
    }
}
