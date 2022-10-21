#![allow(dead_code)]
use super::*;

const JOIN_CHANNELS: [[u32; 2]; 4] = [
    [923_200_000, 923_400_000],
    [921_400_000, 921_600_000],
    [916_600_000, 916_800_000],
    [917_300_000, 917_500_000],
];

const RX2_FREQUENCY: [u32; 4] = [923_200_000, 921_400_000, 916_600_000, 917_300_000];

const AS_DBM: i8 = 16;

mod datarates;
use datarates::*;

pub enum AS923Subband {
    _1,
    _2,
    _3,
    _4,
}

#[allow(clippy::upper_case_acronyms)]
pub struct AS923 {
    subband: AS923Subband,
    last_tx: usize,
    cf_list: Option<[u32; 5]>,
}

impl AS923 {
    pub fn new(subband: AS923Subband) -> AS923 {
        Self {
            subband,
            last_tx: 0,
            cf_list: None,
        }
    }
}

use super::JoinAccept;

impl RegionHandler for AS923 {
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
            DATARATES[datarate as usize].clone(),
            match frame {
                Frame::Data => {
                    if let Some(cf_list) = self.cf_list {
                        let channel = random as usize & 0b111;
                        self.last_tx = channel;
                        if channel < JOIN_CHANNELS[self.subband as usize].len() {
                            JOIN_CHANNELS[self.subband as usize][channel]
                        } else {
                            cf_list[channel - JOIN_CHANNELS[self.subband as usize].len()]
                        }
                    } else {
                        let channel = random as usize % JOIN_CHANNELS[self.subband as usize]len();
                        self.last_tx = channel;
                        JOIN_CHANNELS[self.subband as usize][channel]
                    }
                }
                Frame::Join => {
                    let channel = random as usize % JOIN_CHANNELS[self.subband as usize].len();
                    self.last_tx = channel;
                    JOIN_CHANNELS[self.subband as usize][channel]
                }
            },
        )
    }

    fn get_rx_frequency(&self, _frame: &Frame, window: &Window) -> u32 {
        match window {
            Window::_1 => {
                let channel = self.last_tx;
                if let Some(cf_list) = self.cf_list {
                    if channel < JOIN_CHANNELS[self.subband as usize].len() {
                        JOIN_CHANNELS[self.subband as usize][channel]
                    } else {
                        cf_list[channel - JOIN_CHANNELS[self.subband as usize].len()]
                    }
                } else {
                    JOIN_CHANNELS[self.subband as usize][channel]
                }
            }
            Window::_2 => RX2_FREQUENCY[self.subband as usize],
        }
    }

    fn get_dbm(&self) -> i8 {
        AS_DBM
    }

    fn get_rx_datarate(&self, datarate: DR, _frame: &Frame, window: &Window) -> Datarate {
        let datarate = match window {
            Window::_1 => datarate,
            Window::_2 => DR::_2,
        };
        DATARATES[datarate as usize].clone()
    }
}
