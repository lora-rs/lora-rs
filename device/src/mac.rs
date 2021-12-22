/*
This a temporary design where flags will be left about desired MAC uplinks by the stack
During Uplink assembly, this struct will be inquired to drive construction
 */

use heapless::Vec;

use super::region;
use lorawan_encoding::maccommands::{
    LinkADRAnsPayload, MacCommand, RXTimingSetupAnsPayload, RXTimingSetupReqPayload,
};

#[derive(Default, Debug)]
pub struct Mac {
    adr_ans: AdrAns,
    rx_delay_ans: RxDelayAns,
}

type AdrAns = u8;
type RxDelayAns = bool;

//work around for E0390
trait AdrAnsTrait {
    fn add(&mut self);
    fn clear(&mut self);
    fn get(&self) -> u8;
}

impl AdrAnsTrait for AdrAns {
    fn add(&mut self) {
        *self += 1;
    }
    fn clear(&mut self) {
        *self = 0;
    }
    fn get(&self) -> u8 {
        *self
    }
}

impl AdrAnsTrait for RxDelayAns {
    fn add(&mut self) {
        *self = true;
    }
    fn clear(&mut self) {
        *self = false;
    }
    fn get(&self) -> u8 {
        if *self {
            1
        } else {
            0
        }
    }
}

pub fn del_to_delay_ms(del: u8) -> u32 {
    match del {
        2..=15 => del as u32 * 1000,
        _ => region::constants::RECEIVE_DELAY1,
    }
}

impl Mac {
    pub fn handle_downlink_macs(
        &mut self,
        region: &mut region::Configuration,
        cmds: &mut lorawan_encoding::maccommands::MacCommandIterator,
    ) {
        for cmd in cmds {
            match cmd {
                MacCommand::LinkADRReq(payload) => {
                    // we ignore DR and TxPwr
                    region.set_channel_mask(payload.channel_mask());
                    self.adr_ans.add();
                }
                MacCommand::RXTimingSetupReq(payload) => {
                    region.set_receive_delay1(del_to_delay_ms(payload.delay()));
                    self.ack_rx_delay();
                }
                _ => (),
            }
        }
    }

    pub fn ack_rx_delay(&mut self) {
        self.rx_delay_ans.add();
    }

    pub fn get_cmds(&mut self, macs: &mut Vec<MacCommand, 8>) {
        for _ in 0..self.adr_ans.get() {
            macs.push(MacCommand::LinkADRAns(
                LinkADRAnsPayload::new(&[0x07]).unwrap(),
            ))
            .unwrap();
        }
        self.adr_ans.clear();

        if self.rx_delay_ans.get() == 1 {
            macs.push(MacCommand::RXTimingSetupAns(
                RXTimingSetupAnsPayload::new(&[]).unwrap(),
            ))
            .unwrap();
        }
        self.rx_delay_ans.clear();
    }
}
