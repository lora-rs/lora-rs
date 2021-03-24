#![allow(dead_code)]
use super::*;

const JOIN_CHANNELS: [u32; 3] = [868_100_000, 868_300_000, 868_500_000];

#[derive(Default)]
pub struct EU868 {
    subband: Option<u8>,
    last_tx: usize,
    cf_list: Option<[u32; 5]>,
}

impl EU868 {
    pub fn new() -> EU868 {
        Self::default()
    }
}

use super::JoinAccept;

impl RegionHandler for EU868 {
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
        let channel = random as usize % JOIN_CHANNELS.len();
        self.last_tx = channel;
        JOIN_CHANNELS[channel]
    }

    fn get_data_frequency(&mut self, random: u8) -> u32 {
        if let Some(cf_list) = self.cf_list {
            let channel = random as usize & 0b111;
            if channel < JOIN_CHANNELS.len() {
                JOIN_CHANNELS[channel]
            } else {
                cf_list[channel - JOIN_CHANNELS.len()]
            }
        } else {
            let channel = random as usize % JOIN_CHANNELS.len();
            JOIN_CHANNELS[channel]
        }
    }

    fn get_join_accept_frequency1(&self) -> u32 {
        JOIN_CHANNELS[self.last_tx]
    }

    fn get_rxwindow1_frequency(&self) -> u32 {
        JOIN_CHANNELS[self.last_tx]
    }
}
