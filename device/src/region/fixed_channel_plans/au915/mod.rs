use super::*;

mod frequencies;
use frequencies::*;

mod datarates;
use datarates::*;

const AU_DBM: i8 = 21;
const DEFAULT_RX2: u32 = 923_300_000;

pub(crate) type AU915 = FixedChannelPlan<16, AU915Region>;

#[derive(Default, Clone)]
pub(crate) struct AU915Region;

impl FixedChannelRegion<16> for AU915Region {
    fn datarates() -> &'static [Option<Datarate>; 16] {
        &DATARATES
    }
    fn uplink_channels() -> &'static [u32; 72] {
        &UPLINK_CHANNEL_MAP
    }
    fn downlink_channels() -> &'static [u32; 8] {
        &DOWNLINK_CHANNEL_MAP
    }
    fn get_default_rx2() -> u32 {
        DEFAULT_RX2
    }
    fn get_rx_datarate(tx_datarate: DR, _frame: &Frame, window: &Window) -> Datarate {
        let datarate = match window {
            Window::_1 => {
                // no support for RX1 DR Offset
                match tx_datarate {
                    DR::_0 => DR::_8,
                    DR::_1 => DR::_9,
                    DR::_2 => DR::_10,
                    DR::_3 => DR::_11,
                    DR::_4 => DR::_12,
                    DR::_5 => DR::_13,
                    DR::_6 => DR::_13,
                    DR::_7 => DR::_9,
                    _ => panic!("Invalid TX datarate"),
                }
            }
            Window::_2 => DR::_8,
        };
        DATARATES[datarate as usize].clone().unwrap()
    }
    fn get_dbm() -> i8 {
        AU_DBM
    }
}
