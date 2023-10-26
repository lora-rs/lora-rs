#![allow(dead_code)]
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};

pub(crate) const RECEIVE_DELAY1: u32 = 1000;
pub(crate) const RECEIVE_DELAY2: u32 = RECEIVE_DELAY1 + 1000; // must be RECEIVE_DELAY + 1 s
pub(crate) const JOIN_ACCEPT_DELAY1: u32 = 5000;
pub(crate) const JOIN_ACCEPT_DELAY2: u32 = 6000;
pub(crate) const MAX_FCNT_GAP: usize = 16384;
pub(crate) const ADR_ACK_LIMIT: usize = 64;
pub(crate) const ADR_ACK_DELAY: usize = 32;
pub(crate) const ACK_TIMEOUT: usize = 2; // random delay between 1 and 3 seconds

pub(crate) const DEFAULT_BANDWIDTH: Bandwidth = Bandwidth::_125KHz;
pub(crate) const DEFAULT_SPREADING_FACTOR: SpreadingFactor = SpreadingFactor::_7;
pub(crate) const DEFAULT_CODING_RATE: CodingRate = CodingRate::_4_5;
pub(crate) const DEFAULT_DBM: i8 = 14;
