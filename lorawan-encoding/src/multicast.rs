use crate::maccommands::{Error, MacCommandIterator, SerializableMacCommand};
use lorawan_macros::CommandHandler;

const MAX_GROUPS: usize = 4;

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
/// Downlink Multicast Messages
pub enum DownlinkMulticastMsg<'a> {
    #[cmd(cid = 0x00, len = 0)]
    PackageVersionReq(PackageVersionReqPayload),
    #[cmd(cid = 0x01, len = 1)]
    McGroupStatusReq(McGroupStatusReqPayload<'a>),
    #[cmd(cid = 0x02, len = 24)]
    McGroupSetupReq(McGroupSetupReqPayload<'a>),
    #[cmd(cid = 0x03, len = 1)]
    McGroupDeleteReq(McGroupDeleteReqPayload<'a>),
    #[cmd(cid = 0x04, len = 10)]
    McClassCSessionReq(McClassCSessionReqPayload<'a>),
    #[cmd(cid = 0x05, len = 10)]
    McClassBSessionReq(McClassBSessionReqPayload<'a>),
}

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
/// Uplink Multicast Messages
pub enum UplinkMulticastMsg<'a> {
    #[cmd(cid = 0x00, len = 2)]
    PackageVersionAns(PackageVersionAnsPayload<'a>),
    #[cmd(cid = 0x01)]
    McGroupStatusAns(McGroupStatusAnsPayload<'a>),
    #[cmd(cid = 0x02, len = 1)]
    McGroupSetupAns(McGroupSetupAnsPayload<'a>),
    #[cmd(cid = 0x03, len = 1)]
    McGroupDeleteAns(McGroupDeleteAnsPayload<'a>),
    #[cmd(cid = 0x04, len = 4)]
    McClassCSessionAns(McClassCSessionAnsPayload<'a>),
    #[cmd(cid = 0x05, len = 4)]
    McClassBSessionAns(McClassBSessionAnsPayload<'a>),
}

impl<'a> McGroupStatusAnsPayload<'a> {
    const ITEM_LEN: usize = 5;
    pub fn new(data: &'a [u8]) -> Result<McGroupStatusAnsPayload<'a>, Error> {
        if data.is_empty() {
            return Err(Error::BufferTooShort);
        }
        let status = data[0];
        let required_len = Self::required_len(status);
        if data.len() < required_len {
            return Err(Error::BufferTooShort);
        }
        Ok(McGroupStatusAnsPayload(&data[0..required_len]))
    }

    pub fn required_len(status: u8) -> usize {
        // |  RFU  | NbTotalGroups | AnsGroupMask |
        // | 1 bit |    3 bits     |    4 bits    |
        // Table 5: McGroupStatusAns
        let nb_total_groups = (status >> 4) & 0x07;
        nb_total_groups as usize * Self::ITEM_LEN
    }

    /// Maximum possible length of the payload
    pub const fn max_len() -> usize {
        MAX_GROUPS * Self::ITEM_LEN
    }

    /// Actual length of this specific payload
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        Self::required_len(self.0[0])
    }
}
