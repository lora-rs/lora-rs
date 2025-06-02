/// IN865 region support (865..867 MHz)
///
/// IN865-867 end-devices SHALL support one of the two following data rate options:
/// 1. DR0 to DR5 (minimum set supported for certification)
/// 2. DR0 to DR5 and DR7
///
/// Current status: DR0..DR5 is supported
use super::*;

const MAX_EIRP: u8 = 30;

pub(crate) type IN865 = DynamicChannelPlan<IN865Region>;

#[derive(Default, Clone)]
pub struct IN865Region;

fn in865_freq_check(f: u32) -> bool {
    (865_000_000..=867_000_000).contains(&f)
}

impl<R: DynamicChannelRegion> DynamicChannelPlan<R> {
    pub fn new_in865() -> Self {
        Self::new(in865_freq_check)
    }
}

impl ChannelRegion for IN865Region {
    fn datarates() -> &'static [Option<Datarate>; NUM_DATARATES as usize] {
        &DATARATES
    }

    fn tx_power_adjust(pw: u8) -> Option<u8> {
        match pw {
            0..=10 => Some(MAX_EIRP - (2 * pw)),
            _ => None,
        }
    }
}

impl DynamicChannelRegion for IN865Region {
    fn join_channels() -> u8 {
        3
    }

    fn default_rx2_freq() -> u32 {
        866_550_000
    }

    fn get_rx_datarate(tx_dr: DR, _frame: &Frame, window: &Window) -> Datarate {
        // TODO: Handle RX1 offset
        let dr = match window {
            Window::_1 => match tx_dr {
                DR::_0 | DR::_1 | DR::_2 | DR::_3 | DR::_4 | DR::_5 | DR::_7 => tx_dr,
                DR::_6
                | DR::_8
                | DR::_9
                | DR::_10
                | DR::_11
                | DR::_12
                | DR::_13
                | DR::_14
                | DR::_15 => DR::_0,
            },
            Window::_2 => DR::_2,
        };
        DATARATES[dr as usize].clone().unwrap()
    }

    fn init_channels(channels: &mut ChannelPlan) {
        channels[0] = Some(Channel::new(865_062_500, DR::_0, DR::_5));
        channels[1] = Some(Channel::new(865_402_500, DR::_0, DR::_5));
        channels[2] = Some(Channel::new(865_985_000, DR::_0, DR::_5));
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
    // DR6: RFU
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
