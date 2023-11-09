#![allow(dead_code)]
use super::*;

const JOIN_CHANNELS: [u32; 3] = [865_062_500, 865_402_500, 865_985_000];

pub(crate) type IN865 = DynamicChannelPlan<3, 6, IN865Region>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct IN865Region;

impl ChannelRegion<6> for IN865Region {
    fn datarates() -> &'static [Option<Datarate>; 6] {
        &DATARATES
    }
}

impl DynamicChannelRegion<3, 6> for IN865Region {
    fn join_channels() -> [u32; 3] {
        JOIN_CHANNELS
    }

    fn get_default_rx2() -> u32 {
        866_550_000
    }
}

use super::{Bandwidth, Datarate, SpreadingFactor};

pub(crate) const DATARATES: [Option<Datarate>; 6] = [
    Some(Datarate {
        spreading_factor: SpreadingFactor::_12,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 59,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_11,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 59,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_10,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 59,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_9,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 123,
        max_mac_payload_size_with_dwell_time: 123,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_8,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    Some(Datarate {
        spreading_factor: SpreadingFactor::_7,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    // TODO: ignore FSK data rate for now
];
