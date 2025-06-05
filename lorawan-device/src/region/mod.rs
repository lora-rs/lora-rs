//! LoRaWAN device region definitions (eg: EU868, US915, etc).
use lora_modulation::{Bandwidth, BaseBandModulationParams, CodingRate, SpreadingFactor};
use lorawan::{
    parser::CfList,
    types::{ChannelMask, DataRateRange},
};
use rand_core::RngCore;

use crate::mac::{Frame, Window};
pub(crate) mod constants;
pub(crate) use crate::radio::*;
use constants::*;
// For backward compatibility
pub use lorawan::types::DR;

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

pub(crate) trait ChannelRegion {
    fn datarates() -> &'static [Option<Datarate>; NUM_DATARATES as usize];

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

    fn tx_power_adjust(pw: u8) -> Option<u8>;
}

#[derive(Clone)]
/// Contains LoRaWAN region-specific configuration; is required for creating a LoRaWAN Device.
///
/// Generally constructed using the [`Region`] enum, unless You need to do region-specific
/// fine-tuning, like for example [`US915`] or [`AU915`].
pub struct Configuration {
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Regions supported by this crate: AS923_1, AS923_2, AS923_3, AS923_4, AU915, EU868, EU433, IN865, US915.
///
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
            Region::AS923_1 => State::AS923_1(AS923_1::new_as924()),
            #[cfg(feature = "region-as923-2")]
            Region::AS923_2 => State::AS923_2(AS923_2::new_as924()),
            #[cfg(feature = "region-as923-3")]
            Region::AS923_3 => State::AS923_3(AS923_3::new_as924()),
            #[cfg(feature = "region-as923-4")]
            Region::AS923_4 => State::AS923_4(AS923_4::new_as924_4()),
            #[cfg(feature = "region-au915")]
            Region::AU915 => State::AU915(AU915::default()),
            #[cfg(feature = "region-eu868")]
            Region::EU868 => State::EU868(EU868::new_eu868()),
            #[cfg(feature = "region-eu433")]
            Region::EU433 => State::EU433(EU433::new_eu433()),
            #[cfg(feature = "region-in865")]
            Region::IN865 => State::IN865(IN865::new_in865()),
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

/// This datarate type is used internally for defining [`Bandwidth`]/[`SpreadingFactor`] per
/// region.
#[derive(Debug, Clone)]
pub(crate) struct Datarate {
    pub(crate) bandwidth: Bandwidth,
    pub(crate) spreading_factor: SpreadingFactor,
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
            // We can do this safely, as default output power will be positive
            pw: self.check_tx_power(0).unwrap().unwrap() as i8,
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

    pub(crate) fn get_datarate(&self, dr: u8) -> Option<&Datarate> {
        region_dispatch!(self, get_datarate, dr)
    }

    pub(crate) fn check_tx_power(&self, tx_power: u8) -> Option<Option<u8>> {
        region_dispatch!(self, check_tx_power, tx_power).map(Some)
    }

    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        mut_region_dispatch!(self, get_tx_dr_and_frequency, rng, datarate, frame)
    }

    pub(crate) fn process_join_accept<T: AsRef<[u8]>>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T>,
    ) {
        mut_region_dispatch!(self, process_join_accept, join_accept)
    }

    pub(crate) fn channel_mask_get(&self) -> ChannelMask<9> {
        region_dispatch!(self, channel_mask_get)
    }

    pub(crate) fn channel_mask_set(&mut self, channel_mask: ChannelMask<9>) {
        mut_region_dispatch!(self, channel_mask_set, channel_mask)
    }

    pub(crate) fn channel_mask_update(
        &self,
        channel_mask: &mut ChannelMask<9>,
        ch_mask_ctl: u8,
        ch_mask: ChannelMask<2>,
    ) -> Option<()> {
        region_dispatch!(self, channel_mask_update, channel_mask, ch_mask_ctl, ch_mask)
    }

    pub(crate) fn channel_mask_validate(
        &self,
        channel_mask: &ChannelMask<9>,
        dr: Option<DR>,
    ) -> bool {
        region_dispatch!(self, channel_mask_validate, channel_mask, dr)
    }

    pub(crate) fn get_rx_datarate(&self, tx_dr: DR, rx1_dr_offset: u8, window: &Window) -> DR {
        region_dispatch!(self, get_rx_datarate, tx_dr, rx1_dr_offset, window)
    }

    pub(crate) fn get_rx_frequency(&self, frame: &Frame, window: &Window) -> u32 {
        region_dispatch!(self, get_rx_frequency, frame, window)
    }

    pub(crate) fn get_default_datarate(&self) -> DR {
        region_dispatch!(self, get_default_datarate)
    }

    pub(crate) fn get_coding_rate(&self) -> CodingRate {
        region_dispatch!(self, get_coding_rate)
    }

    pub(crate) fn frequency_valid(&self, f: u32) -> bool {
        region_dispatch!(self, frequency_valid, f)
    }

    #[allow(dead_code)]
    pub(crate) fn get_current_region(&self) -> super::region::Region {
        self.state.region()
    }

    pub(crate) fn has_fixed_channel_plan(&self) -> bool {
        region_dispatch!(self, has_fixed_channel_plan)
    }

    pub(crate) fn channel_dl_update(&mut self, index: u8, freq: u32) -> (bool, bool) {
        mut_region_dispatch!(self, channel_dl_update, index, freq)
    }

    pub(crate) fn handle_new_channel(
        &mut self,
        index: u8,
        freq: u32,
        data_rates: Option<DataRateRange>,
    ) -> (bool, bool) {
        mut_region_dispatch!(self, handle_new_channel, index, freq, data_rates)
    }

    pub(crate) fn rx1_dr_offset_validate(&self, value: u8) -> Option<u8> {
        region_dispatch!(self, rx1_dr_offset_validate, value)
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
    fn process_join_accept<T: AsRef<[u8]>>(&mut self, join_accept: &DecryptedJoinAcceptPayload<T>);

    fn channel_mask_get(&self) -> ChannelMask<9>;
    fn channel_mask_set(&mut self, channel_mask: ChannelMask<9>);

    // TODO: Switch return type to Result
    fn channel_mask_update(
        &self,
        channel_mask: &mut ChannelMask<9>,
        ch_mask_ctl: u8,
        ch_mask: ChannelMask<2>,
    ) -> Option<()>;

    fn channel_mask_validate(&self, channel_mask: &ChannelMask<9>, dr: Option<DR>) -> bool;

    fn channel_dl_update(&mut self, index: u8, freq: u32) -> (bool, bool);

    fn handle_new_channel(
        &mut self,
        index: u8,
        freq: u32,
        data_rates: Option<DataRateRange>,
    ) -> (bool, bool);

    fn get_datarate(&self, dr: u8) -> Option<&Datarate>;

    fn get_default_datarate(&self) -> DR {
        DR::_0
    }

    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32);

    fn get_rx_datarate(&self, datarate: DR, rx1_dr_offset: u8, window: &Window) -> DR;
    fn get_rx_frequency(&self, frame: &Frame, window: &Window) -> u32;
    fn get_coding_rate(&self) -> CodingRate {
        DEFAULT_CODING_RATE
    }

    fn check_tx_power(&self, tx_power: u8) -> Option<u8>;

    fn frequency_valid(&self, freq: u32) -> bool;

    /// Whether region supports modifying channel plan
    /// with `NewChannelReq`/`DlSettingsReq` MAC commands
    fn has_fixed_channel_plan(&self) -> bool;

    fn rx1_dr_offset_validate(&self, value: u8) -> Option<u8>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "region-eu868")]
    fn test_dynamic_region_frequency_range() {
        let r = Configuration::new(Region::EU868);
        assert!(r.frequency_valid(863_000_000));
        assert!(r.frequency_valid(868_000_000));
        assert!(r.frequency_valid(870_000_000));

        assert!(!r.frequency_valid(862_900_000));
        assert!(!r.frequency_valid(870_000_001));

        // Invalid in default eu868 frequency range, but valid in some areas
        assert!(!r.frequency_valid(872_000_000));
    }

    #[test]
    #[cfg(feature = "region-au915")]
    fn test_fixed_au915_frequency_range() {
        let r = Configuration::new(Region::AU915);
        assert!(r.frequency_valid(915_000_000));
        assert!(r.frequency_valid(928_000_000));

        assert!(!r.frequency_valid(902_900_000));
        assert!(!r.frequency_valid(930_000_001));
    }

    #[test]
    #[cfg(feature = "region-us915")]
    fn test_fixed_us915_frequency_range() {
        let r = Configuration::new(Region::US915);
        assert!(r.frequency_valid(902_000_000));
        assert!(r.frequency_valid(915_000_000));
        assert!(r.frequency_valid(928_000_000));

        assert!(!r.frequency_valid(901_900_000));
        assert!(!r.frequency_valid(928_000_001));
    }
}
