//! LoRaWAN device region definitions (eg: EU868, US915, etc).
use lora_modulation::{Bandwidth, BaseBandModulationParams, CodingRate, SpreadingFactor};
use lorawan::{maccommands::ChannelMask, parser::CfList};
use rand_core::RngCore;

use crate::mac::{Frame, Window};
pub(crate) mod constants;
pub(crate) use crate::radio::*;
use constants::*;

#[cfg(not(any(
    feature = "region-as923-1",
    feature = "region-as923-2",
    feature = "region-as923-3",
    feature = "region-as923-4",
    feature = "region-eu433",
    feature = "region-eu868",
    feature = "region-in865",
    feature = "region-au915",
    feature = "region-us915"
)))]
compile_error!("You must enable at least one region! eg: `region-eu868`, `region-us915`...");

#[cfg(any(
    feature = "region-as923-1",
    feature = "region-as923-2",
    feature = "region-as923-3",
    feature = "region-as923-4",
    feature = "region-eu433",
    feature = "region-eu868",
    feature = "region-in865"
))]
mod dynamic_channel_plans;
#[cfg(feature = "region-as923-1")]
pub(crate) use dynamic_channel_plans::AS923_1;
#[cfg(feature = "region-as923-2")]
pub(crate) use dynamic_channel_plans::AS923_2;
#[cfg(feature = "region-as923-3")]
pub(crate) use dynamic_channel_plans::AS923_3;
#[cfg(feature = "region-as923-4")]
pub(crate) use dynamic_channel_plans::AS923_4;
#[cfg(feature = "region-eu433")]
pub(crate) use dynamic_channel_plans::EU433;
#[cfg(feature = "region-eu868")]
pub(crate) use dynamic_channel_plans::EU868;
#[cfg(feature = "region-in865")]
pub(crate) use dynamic_channel_plans::IN865;

#[cfg(any(feature = "region-us915", feature = "region-au915"))]
mod fixed_channel_plans;
#[cfg(any(feature = "region-us915", feature = "region-au915"))]
pub use fixed_channel_plans::Subband;
#[cfg(feature = "region-au915")]
pub use fixed_channel_plans::AU915;
#[cfg(feature = "region-us915")]
pub use fixed_channel_plans::US915;

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
/// Contains LoRaWAN region-specific configuration; is required for creating a LoRaWAN Device.
/// Generally constructed using the `Region` enum, unless you need to fine-tune US915 or AU915.
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
/// Regions supported by this crate: AS923_1, AS923_2, AS923_3, AS923_4, AU915, EU868, EU433, IN865, US915.
/// Each region is individually feature-gated (eg: `region-eu868`), however, by default, all regions are enabled.
///
pub enum Region {
    #[cfg(feature = "region-as923-1")]
    AS923_1,
    #[cfg(feature = "region-as923-2")]
    AS923_2,
    #[cfg(feature = "region-as923-3")]
    AS923_3,
    #[cfg(feature = "region-as923-4")]
    AS923_4,
    #[cfg(feature = "region-au915")]
    AU915,
    #[cfg(feature = "region-eu868")]
    EU868,
    #[cfg(feature = "region-eu433")]
    EU433,
    #[cfg(feature = "region-in865")]
    IN865,
    #[cfg(feature = "region-us915")]
    US915,
}

#[derive(Clone)]
enum State {
    #[cfg(feature = "region-as923-1")]
    AS923_1(AS923_1),
    #[cfg(feature = "region-as923-2")]
    AS923_2(AS923_2),
    #[cfg(feature = "region-as923-3")]
    AS923_3(AS923_3),
    #[cfg(feature = "region-as923-4")]
    AS923_4(AS923_4),
    #[cfg(feature = "region-au915")]
    AU915(AU915),
    #[cfg(feature = "region-eu868")]
    EU868(EU868),
    #[cfg(feature = "region-eu433")]
    EU433(EU433),
    #[cfg(feature = "region-in865")]
    IN865(IN865),
    #[cfg(feature = "region-us915")]
    US915(US915),
}

impl State {
    pub fn new(region: Region) -> State {
        match region {
            #[cfg(feature = "region-as923-1")]
            Region::AS923_1 => State::AS923_1(AS923_1::default()),
            #[cfg(feature = "region-as923-2")]
            Region::AS923_2 => State::AS923_2(AS923_2::default()),
            #[cfg(feature = "region-as923-3")]
            Region::AS923_3 => State::AS923_3(AS923_3::default()),
            #[cfg(feature = "region-as923-4")]
            Region::AS923_4 => State::AS923_4(AS923_4::default()),
            #[cfg(feature = "region-au915")]
            Region::AU915 => State::AU915(AU915::default()),
            #[cfg(feature = "region-eu868")]
            Region::EU868 => State::EU868(EU868::default()),
            #[cfg(feature = "region-eu433")]
            Region::EU433 => State::EU433(EU433::default()),
            #[cfg(feature = "region-in865")]
            Region::IN865 => State::IN865(IN865::default()),
            #[cfg(feature = "region-us915")]
            Region::US915 => State::US915(US915::default()),
        }
    }

    #[allow(dead_code)]
    pub fn region(&self) -> Region {
        match self {
            #[cfg(feature = "region-as923-1")]
            Self::AS923_1(_) => Region::AS923_1,
            #[cfg(feature = "region-as923-2")]
            Self::AS923_2(_) => Region::AS923_2,
            #[cfg(feature = "region-as923-3")]
            Self::AS923_3(_) => Region::AS923_3,
            #[cfg(feature = "region-as923-4")]
            Self::AS923_4(_) => Region::AS923_4,
            #[cfg(feature = "region-au915")]
            Self::AU915(_) => Region::AU915,
            #[cfg(feature = "region-eu433")]
            Self::EU433(_) => Region::EU433,
            #[cfg(feature = "region-eu868")]
            Self::EU868(_) => Region::EU868,
            #[cfg(feature = "region-in865")]
            Self::IN865(_) => Region::IN865,
            #[cfg(feature = "region-us915")]
            Self::US915(_) => Region::US915,
        }
    }
}

/// This datarate type is used internally for defining bandwidth/sf per region
#[derive(Debug, Clone)]
pub(crate) struct Datarate {
    bandwidth: Bandwidth,
    spreading_factor: SpreadingFactor,
    max_mac_payload_size: u8,
    max_mac_payload_size_with_dwell_time: u8,
}
macro_rules! mut_region_dispatch {
  ($s:expr, $t:tt) => {
      match &mut $s.state {
        #[cfg(feature = "region-as923-1")]
        State::AS923_1(state) => state.$t(),
        #[cfg(feature = "region-as923-2")]
        State::AS923_2(state) => state.$t(),
        #[cfg(feature = "region-as923-3")]
        State::AS923_3(state) => state.$t(),
        #[cfg(feature = "region-as923-4")]
        State::AS923_4(state) => state.$t(),
        #[cfg(feature = "region-au915")]
        State::AU915(state) => state.0.$t(),
        #[cfg(feature = "region-eu868")]
        State::EU868(state) => state.$t(),
        #[cfg(feature = "region-eu433")]
        State::EU433(state) => state.$t(),
        #[cfg(feature = "region-in865")]
        State::IN865(state) => state.$t(),
        #[cfg(feature = "region-us915")]
        State::US915(state) => state.0.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &mut $s.state {
        #[cfg(feature = "region-as923-1")]
        State::AS923_1(state) => state.$t($($arg)*),
        #[cfg(feature = "region-as923-2")]
        State::AS923_2(state) => state.$t($($arg)*),
        #[cfg(feature = "region-as923-3")]
        State::AS923_3(state) => state.$t($($arg)*),
        #[cfg(feature = "region-as923-4")]
        State::AS923_4(state) => state.$t($($arg)*),
        #[cfg(feature = "region-au915")]
        State::AU915(state) => state.0.$t($($arg)*),
        #[cfg(feature = "region-eu868")]
        State::EU868(state) => state.$t($($arg)*),
        #[cfg(feature = "region-eu433")]
        State::EU433(state) => state.$t($($arg)*),
        #[cfg(feature = "region-in865")]
        State::IN865(state) => state.$t($($arg)*),
        #[cfg(feature = "region-us915")]
        State::US915(state) => state.0.$t($($arg)*),
    }
  };
}

macro_rules! region_dispatch {
  ($s:expr, $t:tt) => {
      match &$s.state {
        #[cfg(feature = "region-as923-1")]
        State::AS923_1(state) => state.$t(),
        #[cfg(feature = "region-as923-2")]
        State::AS923_2(state) => state.$t(),
        #[cfg(feature = "region-as923-3")]
        State::AS923_3(state) => state.$t(),
        #[cfg(feature = "region-as923-4")]
        State::AS923_4(state) => state.$t(),
        #[cfg(feature = "region-au915")]
        State::AU915(state) => state.0.$t(),
        #[cfg(feature = "region-eu868")]
        State::EU868(state) => state.$t(),
        #[cfg(feature = "region-eu433")]
        State::EU433(state) => state.$t(),
        #[cfg(feature = "region-in865")]
        State::IN865(state) => state.$t(),
        #[cfg(feature = "region-us915")]
        State::US915(state) => state.0.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &$s.state {
        #[cfg(feature = "region-as923-1")]
        State::AS923_1(state) => state.$t($($arg)*),
        #[cfg(feature = "region-as923-2")]
        State::AS923_2(state) => state.$t($($arg)*),
        #[cfg(feature = "region-as923-3")]
        State::AS923_3(state) => state.$t($($arg)*),
        #[cfg(feature = "region-as923-4")]
        State::AS923_4(state) => state.$t($($arg)*),
        #[cfg(feature = "region-au915")]
        State::AU915(state) => state.0.$t($($arg)*),
        #[cfg(feature = "region-eu868")]
        State::EU868(state) => state.$t($($arg)*),
        #[cfg(feature = "region-eu433")]
        State::EU433(state) => state.$t($($arg)*),
        #[cfg(feature = "region-in865")]
        State::IN865(state) => state.$t($($arg)*),
        #[cfg(feature = "region-us915")]
        State::US915(state) => state.0.$t($($arg)*),
    }
  };
}

macro_rules! region_static_dispatch {
  ($s:expr, $t:tt) => {
      match &$s.state {
        #[cfg(feature = "region-as923-1")]
        State::AS923_1(_) => dynamic_channel_plans::AS923_1::$t(),
        #[cfg(feature = "region-as923-2")]
        State::AS923_2(_) => dynamic_channel_plans::AS923_2::$t(),
        #[cfg(feature = "region-as923-3")]
        State::AS923_3(_) => dynamic_channel_plans::AS923_3::$t(),
        #[cfg(feature = "region-as923-4")]
        State::AS923_4(_) => dynamic_channel_plans::AS923_4::$t(),
        #[cfg(feature = "region-au915")]
        State::AU915(_) => fixed_channel_plans::AU915::$t(),
        #[cfg(feature = "region-eu868")]
        State::EU868(_) => dynamic_channel_plans::EU868::$t(),
        #[cfg(feature = "region-eu433")]
        State::EU433(_) => dynamic_channel_plans::EU433::$t(),
        #[cfg(feature = "region-in865")]
        State::IN865(_) => dynamic_channel_plans::IN865::$t(),
        #[cfg(feature = "region-us915")]
        State::US915(_) => fixed_channel_plans::US915::$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &$s.state {
        #[cfg(feature = "region-as923-1")]
        State::AS923_1(_) => dynamic_channel_plans::AS923_1::$t($($arg)*),
        #[cfg(feature = "region-as923-2")]
        State::AS923_2(_) => dynamic_channel_plans::AS923_2::$t($($arg)*),
        #[cfg(feature = "region-as923-3")]
        State::AS923_3(_) => dynamic_channel_plans::AS923_3::$t($($arg)*),
        #[cfg(feature = "region-as923-4")]
        State::AS923_4(_) => dynamic_channel_plans::AS923_4::$t($($arg)*),
        #[cfg(feature = "region-au915")]
        State::AU915(_) => fixed_channel_plans::AU915::$t($($arg)*),
        #[cfg(feature = "region-eu868")]
        State::EU868(_) => dynamic_channel_plans::EU868::$t($($arg)*),
        #[cfg(feature = "region-eu433")]
        State::EU433(_) => dynamic_channel_plans::EU433::$t($($arg)*),
        #[cfg(feature = "region-in865")]
        State::IN865(_) => dynamic_channel_plans::IN865::$t($($arg)*),
        #[cfg(feature = "region-us915")]
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

#[cfg(feature = "region-as923-1")]
from_region!(AS923_1);
#[cfg(feature = "region-as923-2")]
from_region!(AS923_2);
#[cfg(feature = "region-as923-3")]
from_region!(AS923_3);
#[cfg(feature = "region-as923-4")]
from_region!(AS923_4);
#[cfg(feature = "region-in865")]
from_region!(IN865);
#[cfg(feature = "region-au915")]
from_region!(AU915);
#[cfg(feature = "region-eu868")]
from_region!(EU868);
#[cfg(feature = "region-eu433")]
from_region!(EU433);
#[cfg(feature = "region-us915")]
from_region!(US915);

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
