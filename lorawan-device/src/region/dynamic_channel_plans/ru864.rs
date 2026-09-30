/// RU864 region support (864..870 MHz)
///
/// RU864-870 end-devices SHALL support one of the two following data rate options:
/// 1. DR0 to DR5 (minimum set supported for certification)
/// 2. DR0 to DR7
///
/// Current status: DR0..DR5 (minimum set is supported)
use super::*;

const MAX_EIRP: u8 = 16;

pub(crate) type RU864 = DynamicChannelPlan<RU864Region>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct RU864Region;

fn ru864_freq_check(f: u32) -> bool {
    (864_000_000..=870_000_000).contains(&f)
}

impl<R: DynamicChannelRegion> DynamicChannelPlan<R> {
    pub fn new_ru864() -> Self {
        Self::new(ru864_freq_check)
    }
}

impl ChannelRegion for RU864Region {
    const DEFAULT_RX2_FREQ: u32 = 869_100_000;
    const MAX_RX1_DR_OFFSET: u8 = 5;

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

impl DynamicChannelRegion for RU864Region {
    const NUM_JOIN_CHANNELS: u8 = 2;

    fn get_rx_datarate(tx_dr: DR, rx1_dr_offset: u8, window: &Window) -> DR {
        match window {
            Window::_1 => {
                let dr = match tx_dr {
                    DR::_0 | DR::_1 | DR::_2 | DR::_3 | DR::_4 | DR::_5 | DR::_6 | DR::_7 => tx_dr,
                    DR::_8 | DR::_9 | DR::_10 | DR::_11 | DR::_12 | DR::_13 | DR::_14 | DR::_15 => {
                        DR::_0
                    }
                };
                dr.offset_sub(rx1_dr_offset)
            }
            Window::_2 => DR::_0,
        }
    }

    fn init_channels(channels: &mut ChannelPlan) {
        channels[0] = Some(Channel::new(868_900_000, DR::_0, DR::_5));
        channels[1] = Some(Channel::new(869_100_000, DR::_0, DR::_5));
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
    // TODO: DR6: SF7 250 kHz, disabled as in EU868
    None,
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
