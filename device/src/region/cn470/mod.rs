#![allow(dead_code)]
use super::*;
use lorawan_encoding::maccommands::ChannelMask;

mod frequencies;
use frequencies::*;

#[derive(Default)]
pub struct CN470 {
    last_tx: u8,
    cf_list: Option<[u32; 5]>,
}

impl CN470 {
    pub fn new() -> CN470 {
        Self::default()
    }
}

use super::JoinAccept;

impl RegionHandler for CN470 {
    fn process_join_accept<T: core::convert::AsRef<[u8]>, C>(
        &mut self,
        join_accept: &super::DecryptedJoinAcceptPayload<T, C>,
    ) -> JoinAccept {
        let mut new_cf_list = [0, 0, 0, 0, 0];
        if let Some(cf_list) = join_accept.c_f_list() {
            for (index, freq) in cf_list.iter().enumerate() {
                new_cf_list[index] = freq.value();
            }
        }
        self.cf_list = Some(new_cf_list);
        JoinAccept {
            cflist: Some(new_cf_list),
        }
    }

    fn set_channel_mask(&mut self, _chmask: ChannelMask) {
        // one day this should truly be handled
    }

    // no subband setting for CN470
    fn set_subband(&mut self, _subband: u8) {}

    fn get_join_frequency(&mut self, random: u8) -> u32 {
        let channel = random % 96;
        self.last_tx = channel;
        UPLINK_MAP[channel as usize]
    }

    fn get_data_frequency(&mut self, random: u8) -> u32 {
        let channel = random & 0b111;
        self.last_tx = channel;
        UPLINK_MAP[channel as usize]
    }

    fn get_join_accept_frequency1(&self) -> u32 {
        DOWNLINK_MAP[self.last_tx as usize % 2]
    }

    fn get_rxwindow1_frequency(&self) -> u32 {
        DOWNLINK_MAP[self.last_tx as usize % 2]
    }
}
