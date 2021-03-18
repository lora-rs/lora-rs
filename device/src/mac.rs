/*
This a temporary design where flags will be left about desired MAC uplinks by the stack
During Uplink assembly, this struct will be inquired to drive construction
 */

use heapless::consts::*;
use heapless::Vec;

use super::region::{self, RegionHandler};
use lorawan_encoding::maccommands::{LinkADRAnsPayload, MacCommand};

#[derive(Default, Debug)]
pub struct Mac {
    adr_ans: AdrAns,
}

type AdrAns = u8;

//work around for E0390
trait AdrAnsTrait {
    fn add(&mut self);
    fn clear(&mut self);
    fn get(&mut self) -> u8;
}

impl AdrAnsTrait for AdrAns {
    fn add(&mut self) {
        *self += 1;
    }
    fn clear(&mut self) {
        *self = 0;
    }
    fn get(&mut self) -> u8 {
        *self
    }
}

impl Mac {
    pub fn handle_downlink_macs(
        &mut self,
        region: &mut region::Configuration,
        cmds: &mut lorawan_encoding::maccommands::MacCommandIterator,
    ) {
        for cmd in cmds {
            if let MacCommand::LinkADRReq(payload) = cmd {
                // we ignore DR and TxPwr
                region.set_channel_mask(payload.channel_mask());
                self.adr_ans.add();
            }
        }
    }

    pub fn get_cmds(&mut self, macs: &mut Vec<MacCommand, U8>) {
        for _ in 0..self.adr_ans.get() {
            macs.push(MacCommand::LinkADRAns(
                LinkADRAnsPayload::new(&[0x07]).unwrap(),
            ))
            .unwrap();
        }
        self.adr_ans.clear();
    }
}
