/*
This a temporary design where flags will be left about desired MAC uplinks by the stack
During Uplink assembly, this struct will be inquired to drive construction
 */
use heapless::Vec;
use lorawan::maccommands::{LinkADRAnsPayload, RXTimingSetupAnsPayload, UplinkMacCommand};

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Uplink {
    pub adr_ans: AdrAns,
    pub rx_delay_ans: RxDelayAns,
    confirmed: bool,
}

// multiple AdrAns may happen per downlink
// so we aggregate how many AdrAns are required
type AdrAns = u8;
// only one RxDelayReq will happen
// so we only need to implement this as a bool
type RxDelayAns = bool;

//work around for E0390
pub(crate) trait MacAnsTrait {
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

    pub fn ack_rx_delay(&mut self) {
        self.rx_delay_ans.add();
    }

    pub fn get_cmds(&mut self, macs: &mut Vec<UplinkMacCommand, 8>) {
        for _ in 0..self.adr_ans.get() {
            macs.push(UplinkMacCommand::LinkADRAns(LinkADRAnsPayload::new(&[0x07]).unwrap()))
                .unwrap();
        }
        self.adr_ans.clear();

        if self.rx_delay_ans.get() != 0 {
            macs.push(UplinkMacCommand::RXTimingSetupAns(RXTimingSetupAnsPayload::new(&[])))
                .unwrap();
        }
        self.rx_delay_ans.clear();
    }
}
