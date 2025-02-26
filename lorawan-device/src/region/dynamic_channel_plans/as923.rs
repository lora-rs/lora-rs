/// AS923 region support (915..928 MHz)
///
/// AS923 end-devices SHALL support one of the two following data rate options:
/// 1. DR0 to DR5 (minimum set supported for certification)
/// 2. DR0 to DR7
///
/// Current status: DR0..DR6 is supported
use super::*;

const JOIN_CHANNELS: [u32; 2] = [923200000, 923200000];

pub(crate) type AS923_1 = DynamicChannelPlan<2, AS923Region<923_200_000, 0>>;
pub(crate) type AS923_2 = DynamicChannelPlan<2, AS923Region<921_400_000, 1800000>>;
pub(crate) type AS923_3 = DynamicChannelPlan<2, AS923Region<916_600_000, 6600000>>;
pub(crate) type AS923_4 = DynamicChannelPlan<2, AS923Region<917_300_000, 5900000>>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct AS923Region<const DEFAULT_RX2: u32, const O: u32>;

impl<const DEFAULT_RX2: u32, const OFFSET: u32> ChannelRegion for AS923Region<DEFAULT_RX2, OFFSET> {
    fn datarates() -> &'static [Option<Datarate>; NUM_DATARATES as usize] {
        &DATARATES
    }
}

impl<const DEFAULT_RX2: u32, const OFFSET: u32> DynamicChannelRegion<2>
    for AS923Region<DEFAULT_RX2, OFFSET>
{
    fn join_channels() -> [u32; 2] {
        [JOIN_CHANNELS[0] + OFFSET, JOIN_CHANNELS[1] + OFFSET]
    }

    fn get_default_rx2() -> u32 {
        DEFAULT_RX2
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
