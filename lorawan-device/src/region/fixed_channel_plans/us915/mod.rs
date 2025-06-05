/// US915 region support (902..928 MHz)
///
/// US902-928 end-devices SHALL support one of the two following data rate options:
/// 1. DR0 to DR4 and DR8 to DR13 (minimum set supported for certification)
/// 2. DR0 to DR13 (all data rates implemented)
///
/// Current status: DR5 and DR6 are unimplemented (LR-FHSS)
use super::*;

mod frequencies;
use frequencies::*;

mod datarates;
use datarates::*;

const US_DBM: u8 = 21;
const MAX_EIRP: u8 = 30;
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
#[derive(Clone)]
pub struct US915(pub(crate) FixedChannelPlan<US915Region>);

impl US915 {
    pub fn get_max_payload_length(datarate: DR, repeater_compatible: bool, dwell_time: bool) -> u8 {
        US915Region::get_max_payload_length(datarate, repeater_compatible, dwell_time)
    }
}

fn us915_default_freq(f: u32) -> bool {
    (902_000_000..=928_000_000).contains(&f)
}

impl Default for US915 {
    fn default() -> US915 {
        US915(FixedChannelPlan::new(us915_default_freq))
    }
}

#[derive(Default, Clone)]
pub(crate) struct US915Region;

impl ChannelRegion for US915Region {
    fn datarates() -> &'static [Option<Datarate>; NUM_DATARATES as usize] {
        &DATARATES
    }
    fn tx_power_adjust(pw: u8) -> Option<u8> {
        // Note: FCC regulation requires hopping over at least 50 channels when using
        // maximum output power. This is achieved either when more than 50
        // LoRa/125 kHz channels are enabled and/or when at least one LR-FHSS
        // channel is enabled. It is possible to have end-devices with less channels
        // when limiting the conducted transmit power of the end-device to 21 dBm.
        match pw {
            0..=14 => Some(core::cmp::min(US_DBM, MAX_EIRP - (2 * pw))),
            _ => None,
        }
    }
}

impl FixedChannelRegion for US915Region {
    const MAX_RX1_DR_OFFSET: u8 = 3;

    fn uplink_channels() -> &'static [u32; 72] {
        &UPLINK_CHANNEL_MAP
    }
    fn downlink_channels() -> &'static [u32; 8] {
        &DOWNLINK_CHANNEL_MAP
    }
    fn default_rx2_freq() -> u32 {
        DEFAULT_RX2
    }
    fn get_rx_datarate(tx_datarate: DR, _rx1_dr_offset: u8, window: &Window) -> DR {
        match window {
            Window::_1 => {
                // no support for RX1 DR Offset
                match tx_datarate {
                    DR::_0 => DR::_10,
                    DR::_1 => DR::_11,
                    DR::_2 => DR::_12,
                    DR::_3 => DR::_13,
                    DR::_4 => DR::_13,
                    DR::_5 => DR::_10,
                    DR::_6 => DR::_11,
                    // TODO: Figure out the best fallback DR
                    _ => DR::_10,
                }
            }
            Window::_2 => DR::_8,
        }
    }
}
