#![allow(dead_code)]
use super::*;
use lorawan_encoding::maccommands::ChannelMask;

mod frequencies;
use frequencies::*;

mod datarates;
use datarates::*;

const US_DBM: i8 = 21;

#[derive(Default)]
#[allow(clippy::upper_case_acronyms)]
pub struct US915 {
    subband: Option<u8>,
    last_tx: (u8, u8),
}

impl US915 {
    pub fn new() -> US915 {
        Self::default()
    }
    pub fn subband(subband: u8) -> US915 {
        US915 {
            subband: Some(subband),
            last_tx: (0, 0),
        }
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

    fn get_join_frequency(&mut self, datarate: DR, random: u8) -> u32 {
        let subband_channel = random & 0b111;
        let subband = if datarate == DR::_4 {
            8
        } else if let Some(subband) = &self.subband {
            subband - 1
        } else {
            (random >> 3) & 0b111
        };
        self.last_tx = (subband, subband_channel);
        UPLINK_CHANNEL_MAP[subband as usize][subband_channel as usize]
    }

    fn get_data_frequency(&mut self, datarate: DR, random: u8) -> u32 {
        let subband_channel = random & 0b111;
        let subband = if datarate == DR::_4 {
            8
        } else if let Some(subband) = &self.subband {
            subband - 1
        } else {
            (random >> 3) & 0b111
        };
        self.last_tx = (subband, subband_channel);
        UPLINK_CHANNEL_MAP[subband as usize][subband_channel as usize]
    }

    fn get_rx_frequency(&self, _frame: &Frame, window: &Window) -> u32 {
        match window {
            Window::_1 => DOWNLINK_CHANNEL_MAP[self.last_tx.1 as usize],
            Window::_2 => 923_300_000,
        }
    }

    fn get_dbm(&self) -> i8 {
        US_DBM
    }

    fn get_tx_datarate(&self, datarate: DR, frame: &Frame) -> Datarate {
        let datarate = match frame {
            // datarate for JoinRequest is always 0
            Frame::Join => DR::_0,
            Frame::Data => datarate,
        };
        DATARATES[datarate as usize].clone().unwrap()
    }
    fn get_rx_datarate(&self, tx_datarate: DR, _frame: &Frame, window: &Window) -> Datarate {
        let datarate = match window {
            Window::_1 => {
                // no support for RX1 DR Offset
                match tx_datarate {
                    DR::_0 => DR::_10,
                    DR::_1 => DR::_11,
                    DR::_2 => DR::_12,
                    DR::_3 => DR::_13,
                    DR::_4 => DR::_13,
                    _ => panic!("Invalid TX datarate"),
                }
            }
            Window::_2 => DR::_8,
        };
        DATARATES[datarate as usize].clone().unwrap()
    }
}
