use super::*;

mod frequencies;
use frequencies::*;

mod datarates;
use datarates::*;

const AU_DBM: i8 = 21;
const DEFAULT_RX2: u32 = 923_300_000;

/// State struct for the `AU915` region. This struct may be created directly if you wish to fine-tune some parameters.
/// At this time specifying a bias for the subband used during the join process is supported using
/// [`set_join_bias`](Self::set_join_bias) and [`set_join_bias_and_noncompliant_retries`](Self::set_join_bias_and_noncompliant_retries)
/// is suppored. This struct can then be turned into a [`Configuration`] as it implements [`Into<Configuration>`].
///
/// # Note:
///
/// Only [`US915`] and [`AU915`] can be created using this method, because they are the only ones which have
/// parameters that may be fine-tuned at the region level. To create a [`Configuration`] for other regions, use
/// [`Configuration::new`] and specify the region using the [`Region`] enum.
///
/// # Example: Setting up join bias
///
/// ```
/// use lorawan_device::region::{Configuration, AU915, Subband};
///
/// let mut au915 = AU915::new();
/// // Subband 2 is commonly used for The Things Network.
/// au915.set_join_bias(Subband::_2);
/// let configuration: Configuration = au915.into();
/// ```
#[derive(Default, Clone)]
pub struct AU915(pub(crate) FixedChannelPlan<16, AU915Region>);

impl AU915 {
    pub fn get_max_payload_length(datarate: DR, repeater_compatible: bool, dwell_time: bool) -> u8 {
        AU915Region::get_max_payload_length(datarate, repeater_compatible, dwell_time)
    }
}

#[derive(Default, Clone)]
pub(crate) struct AU915Region;

impl ChannelRegion<16> for AU915Region {
    fn datarates() -> &'static [Option<Datarate>; 16] {
        &DATARATES
    }
}

impl FixedChannelRegion<16> for AU915Region {
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
