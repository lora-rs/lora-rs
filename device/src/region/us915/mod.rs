#![allow(dead_code)]
use super::*;
use lorawan_encoding::maccommands::ChannelMask;

mod frequencies;
use frequencies::*;

const US_DBM: i8 = 21;

#[derive(Default)]
pub struct US915 {
    subband: Option<u8>,
    last_tx: (u8, u8),
}

impl US915 {
    pub fn new() -> US915 {
        Self::default()
    }
}

use super::JoinAccept;

impl RegionHandler for US915 {
    fn process_join_accept<T: core::convert::AsRef<[u8]>, C>(
        &mut self,
        _join_accept: &super::DecryptedJoinAcceptPayload<T, C>,
    ) -> JoinAccept {
        JoinAccept { cflist: None }
    }

    fn set_channel_mask(&mut self, _chmask: ChannelMask) {
        // one day this should truly be handled
    }

    fn set_subband(&mut self, subband: u8) {
        self.subband = Some(subband);
    }

    fn get_join_frequency(&mut self, random: u8) -> u32 {
        let subband_channel = random & 0b111;
        let subband = if let Some(subband) = &self.subband {
            subband - 1
        } else {
            (random >> 3) & 0b111
        };
        self.last_tx = (subband, subband_channel);
        UPLINK_CHANNEL_MAP[subband as usize][subband_channel as usize]
    }

    fn get_data_frequency(&mut self, random: u8) -> u32 {
        let subband_channel = random & 0b111;
        let subband = if let Some(subband) = &self.subband {
            subband - 1
        } else {
            (random >> 3) & 0b111
        };
        self.last_tx = (subband, subband_channel);
        UPLINK_CHANNEL_MAP[subband as usize][subband_channel as usize]
    }

    fn get_join_accept_frequency1(&self) -> u32 {
        DOWNLINK_CHANNEL_MAP[self.last_tx.1 as usize]
    }

    fn get_rxwindow1_frequency(&self) -> u32 {
        DOWNLINK_CHANNEL_MAP[self.last_tx.1 as usize]
    }

    fn get_dbm(&self) -> i8 {
        US_DBM
    }
}
