use super::*;

mod frequencies;
use frequencies::*;

mod datarates;
use datarates::*;

const US_DBM: i8 = 21;
const DEFAULT_RX2: u32 = 923_300_000;

/// State struct for the `US915` region. This struct may be created directly if you wish to fine-tune some parameters.
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
/// use lorawan_device::region::{Configuration, US915, Subband};
///
/// let mut us915 = US915::new();
/// // Subband 2 is commonly used for The Things Network.
/// us915.set_join_bias(Subband::_2);
/// let configuration: Configuration = us915.into();
/// ```
#[derive(Default, Clone)]
pub struct US915(pub(crate) FixedChannelPlan<14, US915Region>);

impl US915 {
    pub fn get_max_payload_length(datarate: DR, repeater_compatible: bool, dwell_time: bool) -> u8 {
        US915Region::get_max_payload_length(datarate, repeater_compatible, dwell_time)
    }
}

#[derive(Default, Clone)]
pub(crate) struct US915Region;

impl ChannelRegion<14> for US915Region {
    fn datarates() -> &'static [Option<Datarate>; 14] {
        &DATARATES
    }
}

impl FixedChannelRegion<14> for US915Region {
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
                    DR::_0 => DR::_10,
                    DR::_1 => DR::_11,
                    DR::_2 => DR::_12,
                    DR::_3 => DR::_13,
                    DR::_4 => DR::_13,
                    _ => panic!("Invalid TX datarate"),
                }
            }
            Window::_2 => DR::_8,
        };
        DATARATES[datarate as usize].clone().unwrap()
    }
    fn get_dbm() -> i8 {
        US_DBM
    }
}
