use lora_modulation::{Bandwidth, BaseBandModulationParams, CodingRate, SpreadingFactor};
use lorawan::{maccommands::ChannelMask, parser::CfList};
use rand_core::RngCore;

use crate::mac::{Frame, Window};
pub mod constants;
pub(crate) use crate::radio::*;
use constants::*;

pub(crate) use dynamic_channel_plans::AS923_1;
pub(crate) use dynamic_channel_plans::AS923_2;
pub(crate) use dynamic_channel_plans::AS923_3;
pub(crate) use dynamic_channel_plans::AS923_4;
pub(crate) use dynamic_channel_plans::EU433;
pub(crate) use dynamic_channel_plans::EU868;
pub(crate) use dynamic_channel_plans::IN865;

mod dynamic_channel_plans;
mod fixed_channel_plans;

pub use fixed_channel_plans::{Subband, AU915, US915};

pub(crate) trait ChannelRegion<const D: usize> {
    fn datarates() -> &'static [Option<Datarate>; D];

    fn get_max_payload_length(datarate: DR, repeater_compatible: bool, dwell_time: bool) -> u8 {
        let Some(Some(dr)) = Self::datarates().get(datarate as usize) else {
            return 0;
        };
        let max_size = if dwell_time {
            dr.max_mac_payload_size_with_dwell_time
        } else {
            dr.max_mac_payload_size
        };
        if repeater_compatible && max_size > 230 {
            230
        } else {
            max_size
        }
    }
}

#[derive(Clone)]
pub struct Configuration {
    state: State,
}

seq_macro::seq!(
    N in 0..=15 {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        #[repr(u8)]
        /// A restricted data rate type that exposes the number of variants to only what _may_ be
        /// potentially be possible. Note that not all data rates are valid in all regions.
        pub enum DR {
            #(
                _~N = N,
            )*
        }
    }
);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Region {
    AS923_1,
    AS923_2,
    AS923_3,
    AS923_4,
    AU915,
    EU868,
    EU433,
    IN865,
    US915,
}

#[derive(Clone)]
enum State {
    AS923_1(AS923_1),
    AS923_2(AS923_2),
    AS923_3(AS923_3),
    AS923_4(AS923_4),
    AU915(AU915),
    EU868(EU868),
    EU433(EU433),
    IN865(IN865),
    US915(US915),
}

impl State {
    pub fn new(region: Region) -> State {
        match region {
            Region::AS923_1 => State::AS923_1(AS923_1::default()),
            Region::AS923_2 => State::AS923_2(AS923_2::default()),
            Region::AS923_3 => State::AS923_3(AS923_3::default()),
            Region::AS923_4 => State::AS923_4(AS923_4::default()),
            Region::AU915 => State::AU915(AU915::default()),
            Region::EU868 => State::EU868(EU868::default()),
            Region::EU433 => State::EU433(EU433::default()),
            Region::IN865 => State::IN865(IN865::default()),
            Region::US915 => State::US915(US915::default()),
        }
    }

    #[allow(dead_code)]
    pub fn region(&self) -> Region {
        match self {
            Self::AS923_1(_) => Region::AS923_1,
            Self::AS923_2(_) => Region::AS923_2,
            Self::AS923_3(_) => Region::AS923_3,
            Self::AS923_4(_) => Region::AS923_4,
            Self::AU915(_) => Region::AU915,
            Self::EU433(_) => Region::EU433,
            Self::EU868(_) => Region::EU868,
            Self::IN865(_) => Region::IN865,
            Self::US915(_) => Region::US915,
        }
    }
}

// This datarate type is used internally for defining bandwidth/sf per region
#[derive(Debug, Clone)]
pub struct Datarate {
    bandwidth: Bandwidth,
    spreading_factor: SpreadingFactor,
    max_mac_payload_size: u8,
    max_mac_payload_size_with_dwell_time: u8,
}
macro_rules! mut_region_dispatch {
  ($s:expr, $t:tt) => {
      match &mut $s.state {
        State::AS923_1(state) => state.$t(),
        State::AS923_2(state) => state.$t(),
        State::AS923_3(state) => state.$t(),
        State::AS923_4(state) => state.$t(),
        State::AU915(state) => state.0.$t(),
        State::EU868(state) => state.$t(),
        State::EU433(state) => state.$t(),
        State::IN865(state) => state.$t(),
        State::US915(state) => state.0.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &mut $s.state {
        State::AS923_1(state) => state.$t($($arg)*),
        State::AS923_2(state) => state.$t($($arg)*),
        State::AS923_3(state) => state.$t($($arg)*),
        State::AS923_4(state) => state.$t($($arg)*),
        State::AU915(state) => state.0.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
        State::EU433(state) => state.$t($($arg)*),
        State::IN865(state) => state.$t($($arg)*),
        State::US915(state) => state.0.$t($($arg)*),
    }
  };
}

macro_rules! region_dispatch {
  ($s:expr, $t:tt) => {
      match &$s.state {
        State::AS923_1(state) => state.$t(),
        State::AS923_2(state) => state.$t(),
        State::AS923_3(state) => state.$t(),
        State::AS923_4(state) => state.$t(),
        State::AU915(state) => state.0.$t(),
        State::EU868(state) => state.$t(),
        State::EU433(state) => state.$t(),
        State::IN865(state) => state.$t(),
        State::US915(state) => state.0.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &$s.state {
        State::AS923_1(state) => state.$t($($arg)*),
        State::AS923_2(state) => state.$t($($arg)*),
        State::AS923_3(state) => state.$t($($arg)*),
        State::AS923_4(state) => state.$t($($arg)*),
        State::AU915(state) => state.0.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
        State::EU433(state) => state.$t($($arg)*),
        State::IN865(state) => state.$t($($arg)*),
        State::US915(state) => state.0.$t($($arg)*),
    }
  };
}

macro_rules! region_static_dispatch {
  ($s:expr, $t:tt) => {
      match &$s.state {
        State::AS923_1(_) => dynamic_channel_plans::AS923_1::$t(),
        State::AS923_2(_) => dynamic_channel_plans::AS923_2::$t(),
        State::AS923_3(_) => dynamic_channel_plans::AS923_3::$t(),
        State::AS923_4(_) => dynamic_channel_plans::AS923_4::$t(),
        State::AU915(_) => fixed_channel_plans::AU915::$t(),
        State::EU868(_) => dynamic_channel_plans::EU868::$t(),
        State::EU433(_) => dynamic_channel_plans::EU433::$t(),
        State::IN865(_) => dynamic_channel_plans::IN865::$t(),
        State::US915(_) => fixed_channel_plans::US915::$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &$s.state {
        State::AS923_1(_) => dynamic_channel_plans::AS923_1::$t($($arg)*),
        State::AS923_2(_) => dynamic_channel_plans::AS923_2::$t($($arg)*),
        State::AS923_3(_) => dynamic_channel_plans::AS923_3::$t($($arg)*),
        State::AS923_4(_) => dynamic_channel_plans::AS923_4::$t($($arg)*),
        State::AU915(_) => fixed_channel_plans::AU915::$t($($arg)*),
        State::EU868(_) => dynamic_channel_plans::EU868::$t($($arg)*),
        State::EU433(_) => dynamic_channel_plans::EU433::$t($($arg)*),
        State::IN865(_) => dynamic_channel_plans::IN865::$t($($arg)*),
        State::US915(_) => fixed_channel_plans::US915::$t($($arg)*),
    }
  };
}

impl Configuration {
    pub fn new(region: Region) -> Configuration {
        Configuration::with_state(State::new(region))
    }

    fn with_state(state: State) -> Configuration {
        Configuration { state }
    }

    pub fn get_max_payload_length(
        &self,
        datarate: DR,
        repeater_compatible: bool,
        dwell_time: bool,
    ) -> u8 {
        region_static_dispatch!(
            self,
            get_max_payload_length,
            datarate,
            repeater_compatible,
            dwell_time
        )
    }

    pub(crate) fn create_tx_config<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> TxConfig {
        let (dr, frequency) = self.get_tx_dr_and_frequency(rng, datarate, frame);
        TxConfig {
            pw: self.get_dbm(),
            rf: RfConfig {
                frequency,
                bb: BaseBandModulationParams::new(
                    dr.spreading_factor,
                    dr.bandwidth,
                    self.get_coding_rate(),
                ),
            },
        }
    }

    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        mut_region_dispatch!(self, get_tx_dr_and_frequency, rng, datarate, frame)
    }

    pub(crate) fn get_rx_config(&self, datarate: DR, frame: &Frame, window: &Window) -> RfConfig {
        let dr = self.get_rx_datarate(datarate, frame, window);
        RfConfig {
            frequency: self.get_rx_frequency(frame, window),
            bb: BaseBandModulationParams::new(
                dr.spreading_factor,
                dr.bandwidth,
                self.get_coding_rate(),
            ),
        }
    }

    pub(crate) fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) {
        mut_region_dispatch!(self, process_join_accept, join_accept)
    }

    pub(crate) fn set_channel_mask(
        &mut self,
        channel_mask_control: u8,
        channel_mask: ChannelMask<2>,
    ) {
        mut_region_dispatch!(self, handle_link_adr_channel_mask, channel_mask_control, channel_mask)
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

    // Unicast: The RXC parameters are identical to the RX2 parameters, and they use the same
    // channel and data rate. Modifying the RX2 parameters using the appropriate MAC
    // commands also modifies the RXC parameters.
    pub(crate) fn get_rxc_config(&self, datarate: DR) -> RfConfig {
        let dr = self.get_rx_datarate(datarate, &Frame::Data, &Window::_2);
        let frequency = self.get_rx_frequency(&Frame::Data, &Window::_2);
        RfConfig {
            frequency,
            bb: BaseBandModulationParams::new(
                dr.spreading_factor,
                dr.bandwidth,
                self.get_coding_rate(),
            ),
        }
    }

    pub(crate) fn get_dbm(&self) -> i8 {
        region_dispatch!(self, get_dbm)
    }

    pub(crate) fn get_coding_rate(&self) -> CodingRate {
        region_dispatch!(self, get_coding_rate)
    }

    #[allow(dead_code)]
    pub(crate) fn get_current_region(&self) -> super::region::Region {
        self.state.region()
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
from_region!(EU868);
from_region!(EU433);
from_region!(AU915);
from_region!(AS923_1);
from_region!(AS923_2);
from_region!(AS923_3);
from_region!(AS923_4);

use lorawan::parser::DecryptedJoinAcceptPayload;

pub(crate) trait RegionHandler {
    fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    );

    fn handle_link_adr_channel_mask(
        &mut self,
        channel_mask_control: u8,
        channel_mask: ChannelMask<2>,
    );

    fn get_default_datarate(&self) -> DR {
        DR::_0
    }
    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
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
