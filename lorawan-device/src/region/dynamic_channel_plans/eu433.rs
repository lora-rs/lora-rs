/// EU433 region support (MHz)
///
/// EU433 end-devices SHALL support one of the two following data rate options:
/// 1. DR0 to DR5 (minimum set supported for certification)
/// 2. DR0 to DR7
///
/// Current status: DR7 (FSK) is unimplemented
use super::*;

const MAX_EIRP: u8 = 16;

pub(crate) type EU433 = DynamicChannelPlan<3, EU433Region>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct EU433Region;

fn eu433_freq_check(f: u32) -> bool {
    (433_050_000..=434_790_000).contains(&f)
}

impl<const NUM_JOIN_CHANNELS: usize, R: DynamicChannelRegion>
    DynamicChannelPlan<NUM_JOIN_CHANNELS, R>
{
    pub fn new_eu433() -> Self {
        Self::new(eu433_freq_check)
    }
}

impl ChannelRegion for EU433Region {
    fn datarates() -> &'static [Option<Datarate>; NUM_DATARATES as usize] {
        &DATARATES
    }

    fn tx_power_adjust(pw: u8) -> Option<u8> {
        match pw {
            0..=5 => Some(MAX_EIRP - (2 * pw)),
            _ => None,
        }
    }
}

impl DynamicChannelRegion for EU433Region {
    fn num_join_channels() -> u8 {
        3
    }

    fn get_default_rx2() -> u32 {
        434_665_000
    }

    fn init_channels(channels: &mut ChannelPlan) {
        channels[0] = Some(Channel::new(433_175_000, DR::_0, DR::_5));
        channels[1] = Some(Channel::new(433_375_000, DR::_0, DR::_5));
        channels[2] = Some(Channel::new(433_575_000, DR::_0, DR::_5));
    }
}

use super::{Bandwidth, Datarate, SpreadingFactor};

pub(crate) const DATARATES: [Option<Datarate>; NUM_DATARATES as usize] = [
    // DR0
    Some(Datarate {
        spreading_factor: SpreadingFactor::_12,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 0,
    }),
    // DR1
    Some(Datarate {
        spreading_factor: SpreadingFactor::_11,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 0,
    }),
    // DR2
    Some(Datarate {
        spreading_factor: SpreadingFactor::_10,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 123,
        max_mac_payload_size_with_dwell_time: 19,
    }),
    // DR3
    Some(Datarate {
        spreading_factor: SpreadingFactor::_9,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 123,
        max_mac_payload_size_with_dwell_time: 61,
    }),
    // DR4
    Some(Datarate {
        spreading_factor: SpreadingFactor::_8,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 133,
    }),
    // DR5
    Some(Datarate {
        spreading_factor: SpreadingFactor::_7,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    // DR6
    Some(Datarate {
        spreading_factor: SpreadingFactor::_7,
        bandwidth: Bandwidth::_250KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    // TODO: DR7: FSK: 50 kbps
    None,
    // DR8..DR14: RFU
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];
