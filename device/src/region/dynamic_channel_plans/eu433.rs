#![allow(dead_code)]
use super::*;

const JOIN_CHANNELS: [u32; 3] = [433_175_000, 433_375_000, 433_575_000];

pub(crate) type EU433 = DynamicChannelPlan<3, 7, EU433Region>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct EU433Region;

impl DynamicChannelRegion<3, 7> for EU433Region {
    fn join_channels() -> [u32; 3] {
        JOIN_CHANNELS
    }

    fn datarates() -> &'static [Option<Datarate>; 7] {
        &DATARATES
    }

    fn get_default_rx2() -> u32 {
        434_665_000
    }
}

use super::{Bandwidth, Datarate, SpreadingFactor};

pub(crate) const DATARATES: [Option<Datarate>; 7] = [
    Some(Datarate { spreading_factor: SpreadingFactor::_12, bandwidth: Bandwidth::_125KHz }),
    Some(Datarate { spreading_factor: SpreadingFactor::_11, bandwidth: Bandwidth::_125KHz }),
    Some(Datarate { spreading_factor: SpreadingFactor::_10, bandwidth: Bandwidth::_125KHz }),
    Some(Datarate { spreading_factor: SpreadingFactor::_9, bandwidth: Bandwidth::_125KHz }),
    Some(Datarate { spreading_factor: SpreadingFactor::_8, bandwidth: Bandwidth::_125KHz }),
    Some(Datarate { spreading_factor: SpreadingFactor::_7, bandwidth: Bandwidth::_125KHz }),
    Some(Datarate { spreading_factor: SpreadingFactor::_7, bandwidth: Bandwidth::_250KHz }),
];
