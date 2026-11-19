#![allow(dead_code)]
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};

pub(crate) const RECEIVE_DELAY1: u32 = 1000;
pub(crate) const RECEIVE_DELAY2: u32 = RECEIVE_DELAY1 + 1000; // must be RECEIVE_DELAY + 1 s
pub(crate) const JOIN_ACCEPT_DELAY1: u32 = 5000;
pub(crate) const JOIN_ACCEPT_DELAY2: u32 = 6000;
pub(crate) const MAX_FCNT_GAP: usize = 16384;
pub(crate) const ADR_ACK_LIMIT: usize = 64;
pub(crate) const ADR_ACK_DELAY: usize = 32;
pub(crate) const RETRANSMIT_TIMEOUT_MIN_MS: u32 = 1000;
pub(crate) const RETRANSMIT_TIMEOUT_MAX_MS: u32 = 3000;

// Although there are 16 possible slots, last one is not defined as Datarate
pub(crate) const NUM_DATARATES: u8 = 15;
pub(crate) const NUM_CHANNELS_DYNAMIC: u8 = 16;

/// RP002 regional parameters version implemented by this crate.
/// Encoded as it appears in `DutVersionsAns`: revision 2, major 1, minor 0, patch 3.
pub(crate) const RP002_VERSION: [u8; 4] = [2, 1, 0, 3];

// TODO(RP002-1.0.4): Implement the regional changes before reporting [2, 1, 0, 4].
// Required work:
// - Relay support: WOR/ACK channels and relay delay/backoff parameters per region.
// - Extended dynamic channel plans: 24 default channels and up to 80 channels on
//   EU868, KR920, RU864 and IN865.
// - Extended fixed channel plans: user TX/RX channels 72..=79 on US915 and AU915.
// - CFList type 1 for dynamic channel plans: frequency list plus channel mask.
// - ChMaskCntl blocks 1..=4 and an 80-channel-aware block mask for ChMaskCntl 5.
// - Grow the internal channel mask from 9 to 10 bytes to represent 80 channels.
// - Update regional data-rate and channel-plan validation for the new channels.

// TODO(RP002-1.0.5): Implement the regional changes before reporting [2, 1, 0, 5].
// Required work:
// - Relay WOR channel 1 changes from SF9/DR3 to SF7/DR5.
// - Add SF6/SF5 data rates: EU868/RU864/IN865 DR12..=13, US915 DR7..=8
//   with RX DR14, and KR920 DR12..=13.
// - Extend the regional data-rate, backoff, and maximum-payload tables for the
//   new data rates.
// - Relax LinkADRReq data-rate range validation for DRMin=6 and DRMax=0.
// - Add CFList type 1 channel-mask support for KR920.

pub(crate) const DEFAULT_BANDWIDTH: Bandwidth = Bandwidth::_125KHz;
pub(crate) const DEFAULT_SPREADING_FACTOR: SpreadingFactor = SpreadingFactor::_7;
pub(crate) const DEFAULT_CODING_RATE: CodingRate = CodingRate::_4_5;
pub(crate) const DEFAULT_DBM: i8 = 14;
