/*
This a temporary design where flags will be left about desired MAC uplinks by the stack
During Uplink assembly, this struct will be inquired to drive construction
 */

use super::del_to_delay_ms;
use crate::region;
use heapless::Vec;
use lorawan::maccommands::{LinkADRAnsPayload, MacCommand, RXTimingSetupAnsPayload};

#[derive(Default, Debug)]
pub struct Uplink {
    adr_ans: AdrAns,
    rx_delay_ans: RxDelayAns,
    confirmed: bool,
}

// multiple AdrAns may happen per downlink
// so we aggregate how many AdrAns are required
type AdrAns = u8;
// only one RxDelayReq will happen
// so we only need to implement this as a bool
type RxDelayAns = bool;

//work around for E0390
trait MacAnsTrait {
    fn add(&mut self);
    fn clear(&mut self);
    // we use a uint instead of bool because some ADR responses
    // require a counter for state.
    // eg: ADR Req may be batched in a single downlink and require
    // multiple ADR Ans in the next uplink
    fn get(&self) -> u8;
}

impl MacAnsTrait for AdrAns {
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

impl MacAnsTrait for RxDelayAns {
    fn add(&mut self) {
        *self = true;
    }
    fn clear(&mut self) {
        *self = false;
    }
    fn get(&self) -> u8 {
        u8::from(*self)
    }
}

impl Uplink {
    pub fn set_downlink_confirmation(&mut self) {
        self.confirmed = true;
    }

    pub fn clear_downlink_confirmation(&mut self) {
        self.confirmed = false;
    }
    pub fn confirms_downlink(&self) -> bool {
        self.confirmed
    }

    pub fn handle_downlink_macs(
        &mut self,
        region: &mut region::Configuration,
        cmds: &mut lorawan::maccommands::MacCommandIterator,
    ) {
        for cmd in cmds {
            match cmd {
                MacCommand::LinkADRReq(payload) => {
                    // we ignore DR and TxPwr
                    region.set_channel_mask(
                        payload.redundancy().channel_mask_control(),
                        payload.channel_mask(),
                    );
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
            macs.push(MacCommand::LinkADRAns(LinkADRAnsPayload::new(&[0x07]).unwrap())).unwrap();
        }
        self.adr_ans.clear();

        if self.rx_delay_ans.get() != 0 {
            macs.push(MacCommand::RXTimingSetupAns(RXTimingSetupAnsPayload::new(&[]).unwrap()))
                .unwrap();
        }
        self.rx_delay_ans.clear();
    }
}
