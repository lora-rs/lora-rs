use super::*;

const JOIN_CHANNELS: [u32; 2] = [923200000, 923200000];

pub(crate) type AS923_1 = DynamicChannelPlan<2, 7, AS923Region<923_200_000, 0>>;
pub(crate) type AS923_2 = DynamicChannelPlan<2, 7, AS923Region<921_400_000, 1800000>>;
pub(crate) type AS923_3 = DynamicChannelPlan<2, 7, AS923Region<916_600_000, 6600000>>;
pub(crate) type AS923_4 = DynamicChannelPlan<2, 7, AS923Region<917_300_000, 5900000>>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct AS923Region<const DEFAULT_RX2: u32, const O: u32>;

impl<const DEFAULT_RX2: u32, const OFFSET: u32> ChannelRegion<7>
    for AS923Region<DEFAULT_RX2, OFFSET>
{
    fn datarates() -> &'static [Option<Datarate>; 7] {
        &DATARATES
    }
}

impl<const DEFAULT_RX2: u32, const OFFSET: u32> DynamicChannelRegion<2, 7>
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

pub(crate) const DATARATES: [Option<Datarate>; 7] = [
    Some(Datarate {
        spreading_factor: SpreadingFactor::_12,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 0,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_11,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 0,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_10,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 123,
        max_mac_payload_size_with_dwell_time: 19,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_9,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 123,
        max_mac_payload_size_with_dwell_time: 61,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_8,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 133,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_7,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_7,
        bandwidth: Bandwidth::_250KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    // TODO: ignore FSK data rate for now
];
