#![allow(dead_code)]
use super::*;

const JOIN_CHANNELS: [u32; 3] = [433_175_000, 433_375_000, 433_575_000];

mod datarates;
use datarates::*;

#[derive(Default, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct EU433 {
    last_tx: usize,
    cf_list: Option<[u32; 5]>,
}

impl EU433 {
    pub fn new() -> EU433 {
        Self::default()
    }
}

use super::JoinAccept;

impl RegionHandler for EU433 {
    fn process_join_accept<T: core::convert::AsRef<[u8]>, C>(
        &mut self,
        join_accept: &super::DecryptedJoinAcceptPayload<T, C>,
    ) -> JoinAccept {
        let mut new_cf_list = [0, 0, 0, 0, 0];
        if let Some(CfList::DynamicChannel(cf_list)) = join_accept.c_f_list() {
            for (index, freq) in cf_list.iter().enumerate() {
                new_cf_list[index] = freq.value();
            }
        }
        self.cf_list = Some(new_cf_list);
        JoinAccept {
            cflist: Some(new_cf_list),
        }
    }

    fn get_tx_dr_and_frequency(
        &mut self,
        random: u8,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        (
            { DATARATES[datarate as usize].clone() },
            match frame {
                Frame::Data => {
                    if let Some(cf_list) = self.cf_list {
                        let channel = random as usize & 0b111;
                        self.last_tx = channel;
                        if channel < JOIN_CHANNELS.len() {
                            JOIN_CHANNELS[channel]
                        } else {
                            cf_list[channel - JOIN_CHANNELS.len()]
                        }
                    } else {
                        let channel = random as usize % JOIN_CHANNELS.len();
                        self.last_tx = channel;
                        JOIN_CHANNELS[channel]
                    }
                }
                Frame::Join => {
                    let channel = random as usize % JOIN_CHANNELS.len();
                    self.last_tx = channel;
                    JOIN_CHANNELS[channel]
                }
            },
        )
    }

    fn get_rx_frequency(&self, _frame: &Frame, window: &Window) -> u32 {
        match window {
            Window::_1 => {
                let channel = self.last_tx;
                if let Some(cf_list) = self.cf_list {
                    if channel < JOIN_CHANNELS.len() {
                        JOIN_CHANNELS[channel]
                    } else {
                        cf_list[channel - JOIN_CHANNELS.len()]
                    }
                } else {
                    let channel = self.last_tx;
                    JOIN_CHANNELS[channel]
                }
            }
            Window::_2 => 434_665_000,
        }
    }

    fn get_rx_datarate(&self, datarate: DR, _frame: &Frame, window: &Window) -> Datarate {
        let datarate = match window {
            Window::_1 => datarate,
            Window::_2 => DR::_0,
        };
        DATARATES[datarate as usize].clone()
    }
}
