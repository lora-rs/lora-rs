#![allow(dead_code)]

const UPLINK_CHANNEL_MAP: [[u32; 8]; 8] = [
    [
        902300000, 902500000, 902700000, 902900000, 903100000, 903300000, 903500000, 903700000,
    ],
    [
        903900000, 904100000, 904300000, 904500000, 904700000, 904900000, 905100000, 905300000,
    ],
    [
        905500000, 905700000, 905900000, 906100000, 906300000, 906500000, 906700000, 906900000,
    ],
    [
        907100000, 907300000, 907500000, 907700000, 907900000, 908100000, 908300000, 908500000,
    ],
    [
        908700000, 908900000, 909100000, 909300000, 909500000, 909700000, 909900000, 910100000,
    ],
    [
        910300000, 910500000, 910700000, 910900000, 911100000, 911300000, 911500000, 911700000,
    ],
    [
        911900000, 912100000, 912300000, 912500000, 912700000, 912900000, 913100000, 913300000,
    ],
    [
        913500000, 913700000, 913900000, 914100000, 914300000, 914500000, 914700000, 914900000,
    ],
];

const DOWNLINK_CHANNEL_MAP: [u32; 8] = [
    922300000, 923900000, 924500000, 925100000, 925700000, 926300000, 926900000, 927500000,
];

const RECEIVE_DELAY1: usize = 1;
const RECEIVE_DELAY2: usize = 2; // must be RECEIVE_DELAY + 1 s
const JOIN_ACCEPT_DELAY1: usize = 5;
const JOIN_ACCEPT_DELAY2: usize = 6;
const MAX_FCNT_GAP: usize = 16384;
const ADR_ACK_LIMIT: usize = 64;
const ADR_ACK_DELAY: usize = 32;
const ACK_TIMEOUT: usize = 2; // random delay between 1 and 3 seconds

pub struct Configuration {
    subband: Option<u8>,
    last_join: (u8, u8),
}
impl Configuration {
    pub fn new() -> Configuration {
        Configuration {
            subband: None,
            last_join: (0, 0),
        }
    }

    pub fn set_subband(&mut self, subband: u8) {
        self.subband = Some(subband);
    }

    pub fn get_join_frequency(&mut self, random: u8) -> u32 {
        let subband_channel = random & 0b111;
        let subband = if let Some(subband) = &self.subband {
            subband - 1
        } else {
            (random >> 3) & 0b111
        };
        self.last_join = (subband, subband_channel);
        UPLINK_CHANNEL_MAP[subband as usize][subband_channel as usize]
    }

    pub fn get_data_frequency(&mut self, random: u8) -> u32 {
        let subband_channel = random & 0b111;
        let subband = if let Some(subband) = &self.subband {
            subband - 1
        } else {
            (random >> 3) & 0b111
        };
        UPLINK_CHANNEL_MAP[subband as usize][subband_channel as usize]
    }

    pub fn get_join_accept_frequency1(&mut self) -> u32 {
        DOWNLINK_CHANNEL_MAP[self.last_join.1 as usize]
    }

    pub fn get_join_accept_delay1(&mut self) -> usize {
        JOIN_ACCEPT_DELAY1
    }

    pub fn get_join_accept_delay2(&mut self) -> usize {
        JOIN_ACCEPT_DELAY2
    }
}
