/// EU688 region support (863..870 MHz)
///
/// EU863-870 end-devices SHALL support one of the three following data rate options:
/// 1. DR0 to DR5 (minimum set supported for certification)
/// 2. DR0 to DR7
/// 3. DR0 to DR11 (all data rates implemented)
///
/// Current status: DR0..DR5 (minimum set is supported)
use super::*;

const MAX_EIRP: u8 = 16;

pub(crate) type EU868 = DynamicChannelPlan<EU868Region>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct EU868Region;

fn eu868_freq_check(f: u32) -> bool {
    (863_000_000..=870_000_000).contains(&f)
}

impl<R: DynamicChannelRegion> DynamicChannelPlan<R> {
    pub fn new_eu868() -> Self {
        Self::new(eu868_freq_check)
    }
}

impl ChannelRegion for EU868Region {
    fn datarates() -> &'static [Option<Datarate>; NUM_DATARATES as usize] {
        &DATARATES
    }

    fn tx_power_adjust(pw: u8) -> Option<u8> {
        match pw {
            0..=7 => Some(MAX_EIRP - (2 * pw)),
            _ => None,
        }
    }
}

impl DynamicChannelRegion for EU868Region {
    fn num_join_channels() -> u8 {
        3
    }

    fn get_default_rx2() -> u32 {
        869_525_000
    }

    fn init_channels(channels: &mut ChannelPlan) {
        channels[0] = Some(Channel::new(868_100_000, DR::_0, DR::_5));
        channels[1] = Some(Channel::new(868_300_000, DR::_0, DR::_5));
        channels[2] = Some(Channel::new(868_500_000, DR::_0, DR::_5));
    }
}

use super::{Bandwidth, Datarate, SpreadingFactor};

pub(crate) const DATARATES: [Option<Datarate>; NUM_DATARATES as usize] = [
    // DR0
    Some(Datarate {
        spreading_factor: SpreadingFactor::_12,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 59,
    }),
    // DR1
    Some(Datarate {
        spreading_factor: SpreadingFactor::_11,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 59,
    }),
    // DR2
    Some(Datarate {
        spreading_factor: SpreadingFactor::_10,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 59,
        max_mac_payload_size_with_dwell_time: 59,
    }),
    // DR3
    Some(Datarate {
        spreading_factor: SpreadingFactor::_9,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 123,
        max_mac_payload_size_with_dwell_time: 123,
    }),
    // DR4
    Some(Datarate {
        spreading_factor: SpreadingFactor::_8,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    // DR5
    Some(Datarate {
        spreading_factor: SpreadingFactor::_7,
        bandwidth: Bandwidth::_125KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    None,
    /*
    // TODO: DR6: Can be enabled once DR7 is implemented
    Some(Datarate {
        spreading_factor: SpreadingFactor::_7,
        bandwidth: Bandwidth::_250KHz,
        max_mac_payload_size: 250,
        max_mac_payload_size_with_dwell_time: 250,
    }),
    */
    // TODO: DR7: FSK: 50 kbps
    None,
    // TODO: DR8: LR-FHSS CR1/3: 137 kHz BW
    None,
    // TODO: DR9: LR-FHSS CR2/3: 137 kHz BW
    None,
    // TODO: DR10: LR-FHSS CR1/3: 336 kHz BW
    None,
    // TODO: DR11: LR-FHSS CR2/3: 336 kHz BW
    None,
    // DR12..DR14: RFU
    None,
    None,
    None,
];
