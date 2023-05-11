#![allow(clippy::upper_case_acronyms)]
// generally, we allow upper_case_acronyms to make it match the LoRaWAN naming conventions better
use lorawan::{maccommands::ChannelMask, parser::CfList};

use super::RngCore;
pub mod constants;
use crate::mac;
pub(crate) use crate::radio::*;
use constants::*;

pub(crate) use dynamic_channel_plans::AS923_1;
pub(crate) use dynamic_channel_plans::AS923_2;
pub(crate) use dynamic_channel_plans::AS923_3;
pub(crate) use dynamic_channel_plans::AS923_4;
pub(crate) use dynamic_channel_plans::EU433;
pub(crate) use dynamic_channel_plans::EU868;
pub(crate) use dynamic_channel_plans::IN865;

pub(crate) use fixed_channel_plans::AU915;
pub(crate) use fixed_channel_plans::US915;

mod dynamic_channel_plans;
mod fixed_channel_plans;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    UnsupportedChannel,
    ChannelListTooLong,
    UnsupportedRegion,
}

#[derive(Clone)]
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
        State::AS923_1(state) => state.$t(),
        State::AS923_2(state) => state.$t(),
        State::AS923_3(state) => state.$t(),
        State::AS923_4(state) => state.$t(),
        State::AU915(state) => state.$t(),
        State::EU868(state) => state.$t(),
        State::EU433(state) => state.$t(),
        State::IN865(state) => state.$t(),
        State::US915(state) => state.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &mut $s.state {
        State::AS923_1(state) => state.$t($($arg)*),
        State::AS923_2(state) => state.$t($($arg)*),
        State::AS923_3(state) => state.$t($($arg)*),
        State::AS923_4(state) => state.$t($($arg)*),
        State::AU915(state) => state.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
        State::EU433(state) => state.$t($($arg)*),
        State::IN865(state) => state.$t($($arg)*),
        State::US915(state) => state.$t($($arg)*),
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
        State::AU915(state) => state.$t(),
        State::EU868(state) => state.$t(),
        State::EU433(state) => state.$t(),
        State::IN865(state) => state.$t(),
        State::US915(state) => state.$t(),
    }
  };
  ($s:expr, $t:tt, $($arg:tt)*) => {
      match &$s.state {
        State::AS923_1(state) => state.$t($($arg)*),
        State::AS923_2(state) => state.$t($($arg)*),
        State::AS923_3(state) => state.$t($($arg)*),
        State::AS923_4(state) => state.$t($($arg)*),
        State::AU915(state) => state.$t($($arg)*),
        State::EU868(state) => state.$t($($arg)*),
        State::EU433(state) => state.$t($($arg)*),
        State::IN865(state) => state.$t($($arg)*),
        State::US915(state) => state.$t($($arg)*),
    }
  };
}

impl Configuration {
    pub fn new(region: Region) -> Configuration {
        Configuration::with_state(State::new(region))
    }

    /// Create a new [`Configuration`] with a specific set of channels enabled for joining the network.
    ///
    /// When `join` is called on a [`Configuration`] created using this
    /// method, the network will be attempted to be joined only on the provided
    /// channel subset.
    ///
    /// This method only makes sense for fixed channel plans (AU915, US915). Trying to call
    /// this constructor with a dynamic channel region will return `Err(())`.
    ///
    /// # About supported channels (fixed channel plans only)
    ///
    /// Supported channels:
    ///
    /// * 64 125 kHz channels (0-63)
    ///
    /// If a channel out of this range is specified, `Err(())` will be returned.
    ///
    ///
    /// # Note
    ///
    /// It is recommended to try to join the network with a channel bias only a few
    /// times. If joining is unsuccessful, use
    /// [`Device::reset_channels`](crate::async_device::Device) to re-enable all the.
    /// regional plan's channels. The reason for this is if you *only* try to join with,
    /// a channel bias, and the network is configured to use a strictly different set of
    /// channels than the ones you provide, the network can NEVER be joined.
    ///
    /// # Returns
    ///
    /// * `Ok(Configuration) if the provided channel set is correct and the region is a fixed channel plan
    /// * The length of `channel_list` must be <= 72, otherwise `Err(())` will be returned.
    /// * If a channel out of the specified channel range is specified, `Err(())` will be returned (ie, >= 64).
    pub fn with_join_channels(
        region: Region,
        join_channels: &[u8],
    ) -> Result<Configuration, Error> {
        use Region::*;
        match region {
            US915 | AU915 => {
                if join_channels.len() > 64 {
                    return Err(Error::ChannelListTooLong);
                }

                let mut config = Configuration::with_state(State::new(region));
                let empty_mask = ChannelMask::<2>::new_from_raw(&[0x00, 0x00]);
                let mut masks = [
                    empty_mask.clone(),
                    empty_mask.clone(),
                    empty_mask.clone(),
                    empty_mask,
                ];

                // Construct the channel masks from the provided channel list
                for channel in join_channels {
                    if *channel >= 64 {
                        return Err(Error::UnsupportedChannel);
                    }

                    let mask_idx = (channel / 16) as usize;
                    let mask = &mut masks[mask_idx];

                    let bank = (*channel as usize - mask_idx * 16) / 8;
                    let old = mask.get_index(bank);
                    let bit_pos = channel % 8;

                    mask.set_bank(bank, (1 << bit_pos) | old);
                }

                // Set the enabled channels in config
                for (cm_ctrl, mask) in masks.iter().enumerate() {
                    config.set_channel_mask(cm_ctrl as u8, mask.clone());
                }

                Ok(config)
            }
            _ => Err(Error::UnsupportedRegion),
        }
    }

    fn with_state(state: State) -> Configuration {
        Configuration {
            state,
            receive_delay1: RECEIVE_DELAY1,
            receive_delay2: RECEIVE_DELAY2,
            join_accept_delay1: JOIN_ACCEPT_DELAY1,
            join_accept_delay2: JOIN_ACCEPT_DELAY2,
        }
    }

    // RECEIVE_DELAY2 is not configurable. LoRaWAN 1.0.3 Section 5.7: "The second reception slot
    // opens one second after the first reception slot."
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
                bandwidth: dr.bandwidth,
                spreading_factor: dr.spreading_factor,
                coding_rate: self.get_coding_rate(),
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

    pub(crate) fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) {
        self.set_receive_delay1(mac::del_to_delay_ms(join_accept.rx_delay()));
        mut_region_dispatch!(self, process_join_accept, join_accept)
    }

    pub(crate) fn set_channel_mask(
        &mut self,
        channel_mask_control: u8,
        channel_mask: ChannelMask<2>,
    ) {
        mut_region_dispatch!(self, handle_link_adr_channel_mask, channel_mask_control, channel_mask)
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
