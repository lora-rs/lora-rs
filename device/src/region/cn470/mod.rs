#![allow(dead_code)]
use super::*;

mod frequencies;
use frequencies::*;

#[derive(Default)]
pub struct CN470 {
    last_tx: usize,
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

    fn get_join_frequency(&mut self, random: u8) -> u32 {
        let channel = random as usize % UPLINK_MAP.len();
        self.last_tx = channel;
        UPLINK_MAP[channel]
    }

    fn get_data_frequency(&mut self, random: u8) -> u32 {
        let channel = random as usize % UPLINK_MAP.len();
        self.last_tx = channel;
        UPLINK_MAP[channel]
    }

    fn get_join_accept_frequency1(&self) -> u32 {
        DOWNLINK_MAP[self.last_tx as usize % 2]
    }

    fn get_rxwindow1_frequency(&self) -> u32 {
        DOWNLINK_MAP[self.last_tx as usize % 2]
    }
}
