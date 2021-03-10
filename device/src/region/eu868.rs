#![allow(dead_code)]

use crate::radio::{Bandwidth, CodingRate, SpreadingFactor};
use lorawan_encoding::maccommands::ChannelMask;

const UPLINK_CHANNEL_MAP: [u32; 9] = [
    867_100_000,
    867_300_000,
    867_500_000,
    867_700_000,
    867_900_000,
    868_100_000,
    868_300_000,
    868_500_000,
    868_800_000,
];

const DOWNLINK_CHANNEL_MAP: [u32; 10] = [
    867_100_000,
    867_300_000,
    867_500_000,
    867_700_000,
    867_900_000,
    868_100_000,
    868_300_000,
    868_500_000,
    868_800_000,
    869_525_000,
];

const RECEIVE_DELAY1: u32 = 1000;
const RECEIVE_DELAY2: u32 = RECEIVE_DELAY1 + 1000; // must be RECEIVE_DELAY + 1 s
const JOIN_ACCEPT_DELAY1: u32 = 5000;
const JOIN_ACCEPT_DELAY2: u32 = 6000;
const MAX_FCNT_GAP: usize = 16384;
const ADR_ACK_LIMIT: usize = 64;
const ADR_ACK_DELAY: usize = 32;
const ACK_TIMEOUT: usize = 2; // random delay between 1 and 3 seconds
const DEFAULT_BANDWIDTH: Bandwidth = Bandwidth::_125KHZ;
const DEFAULT_SPREADING_FACTOR: SpreadingFactor = SpreadingFactor::_7;
const DEFAULT_CODING_RATE: CodingRate = CodingRate::_4_5;
const DEFAULT_DBM: i8 = 14;

#[derive(Default)]
pub struct Configuration {
    channel: Option<u8>,
    last_tx: u8,
}

impl Configuration {
    pub(crate) fn new() -> Configuration {
        Self::default()
    }

    pub(crate) fn set_channel_mask(&mut self, _chmask: ChannelMask) {
        // one day this should truly be handled
    }

    pub(crate) fn set_subband(&mut self, _subband: u8) {
        // No subband in this region
    }

    pub(crate) fn select_join_frequency(&mut self, random: u8) -> u32 {
        let channel = if let Some(channel) = &self.channel {
            if *channel == 0 {
                random % UPLINK_CHANNEL_MAP.len() as u8
            } else {
                *channel - 1
            }
        } else {
            random % UPLINK_CHANNEL_MAP.len() as u8
        };

        self.channel = Some(channel);
        self.last_tx = channel;
        UPLINK_CHANNEL_MAP[channel as usize]
    }

    pub(crate) fn select_data_frequency(&mut self, random: u8) -> u32 {
        let channel = if let Some(channel) = &self.channel {
            if *channel == 0 {
                random % UPLINK_CHANNEL_MAP.len() as u8
            } else {
                *channel - 1
            }
        } else {
            random % UPLINK_CHANNEL_MAP.len() as u8
        };

        self.channel = Some(channel);
        self.last_tx = channel;
        UPLINK_CHANNEL_MAP[channel as usize]
    }

    pub(crate) fn get_join_accept_frequency1(&self) -> u32 {
        DOWNLINK_CHANNEL_MAP[self.last_tx as usize]
    }

    pub(crate) fn get_rxwindow1_frequency(&self) -> u32 {
        DOWNLINK_CHANNEL_MAP[self.last_tx as usize]
    }

    pub(crate) fn get_bandwidth(&self) -> Bandwidth {
        DEFAULT_BANDWIDTH
    }

    pub(crate) fn get_dbm(&self) -> i8 {
        DEFAULT_DBM
    }

    pub(crate) fn get_coding_rate(&self) -> CodingRate {
        DEFAULT_CODING_RATE
    }

    pub(crate) fn get_spreading_factor(&self) -> SpreadingFactor {
        DEFAULT_SPREADING_FACTOR
    }

    pub(crate) fn get_join_accept_delay1(&self) -> u32 {
        JOIN_ACCEPT_DELAY1
    }

    pub(crate) fn get_join_accept_delay2(&self) -> u32 {
        JOIN_ACCEPT_DELAY2
    }

    pub(crate) fn get_receive_delay1(&self) -> u32 {
        RECEIVE_DELAY1
    }

    pub(crate) fn get_receive_delay2(&self) -> u32 {
        RECEIVE_DELAY2
    }
}
