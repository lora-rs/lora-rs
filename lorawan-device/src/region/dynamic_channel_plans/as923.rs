/// AS923 region support (915..928 MHz)
///
/// AS923 end-devices SHALL support one of the two following data rate options:
/// 1. DR0 to DR5 (minimum set supported for certification)
/// 2. DR0 to DR7
///
/// Current status: DR0..DR6 is supported
use super::*;

const MAX_EIRP: u8 = 16;

pub(crate) type AS923_1 = DynamicChannelPlan<AS923Region<923_200_000, 0>>;
pub(crate) type AS923_2 = DynamicChannelPlan<AS923Region<921_400_000, 1800000>>;
pub(crate) type AS923_3 = DynamicChannelPlan<AS923Region<916_500_000, 6600000>>;
pub(crate) type AS923_4 = DynamicChannelPlan<AS923Region<917_300_000, 5900000>>;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct AS923Region<const DEFAULT_RX2: u32, const O: u32>;

impl<const DEFAULT_RX2: u32, const OFFSET: u32> ChannelRegion for AS923Region<DEFAULT_RX2, OFFSET> {
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

fn as924_generic_freq_check(f: u32) -> bool {
    (915_000_000..=928_000_000).contains(&f)
}

fn as924_4_freq_check(f: u32) -> bool {
    (917_000_000..=920_000_000).contains(&f)
}

impl<R: DynamicChannelRegion> DynamicChannelPlan<R> {
    pub fn new_as924() -> Self {
        Self::new(as924_generic_freq_check)
    }

    pub fn new_as924_4() -> Self {
        Self::new(as924_4_freq_check)
    }
}

impl<const DEFAULT_RX2: u32, const OFFSET: u32> DynamicChannelRegion
    for AS923Region<DEFAULT_RX2, OFFSET>
{
    fn join_channels() -> u8 {
        2
    }

    fn default_rx2_freq() -> u32 {
        DEFAULT_RX2
    }

    fn get_rx_datarate(tx_dr: DR, _frame: &Frame, window: &Window) -> Datarate {
        // TODO: Handle DwellTime, current values correspond to Dwelltime = 0
        // TODO: Handle RX1 offset
        let dr = match window {
            Window::_1 => match tx_dr {
                DR::_0 | DR::_1 | DR::_2 | DR::_3 | DR::_4 | DR::_5 | DR::_6 | DR::_7 => tx_dr,
                DR::_8 | DR::_9 | DR::_10 | DR::_11 | DR::_12 | DR::_13 | DR::_14 | DR::_15 => {
                    DR::_0
                }
            },
            Window::_2 => DR::_2,
        };
        DATARATES[dr as usize].clone().unwrap()
    }

    // Although Network gateways SHALL always listen on following frequencies
    // with DR0..=DR5, the default Join-Request Data Rate SHALL utilize DR2..=DR5
    // (SF10/125 kHz – SF7/125 kHz).
    fn init_channels(channels: &mut ChannelPlan) {
        channels[0] = Some(Channel::new(923200000 - OFFSET, DR::_2, DR::_5));
        channels[1] = Some(Channel::new(923400000 - OFFSET, DR::_2, DR::_5));
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
