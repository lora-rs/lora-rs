mod group_setup;
use crate::maccommands::{Error, MacCommandIterator, SerializableMacCommand};
pub use group_setup::Session;
use lorawan_macros::CommandHandler;

pub const MAX_GROUPS: usize = 4;

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
/// Downlink Multicast Remote Setup Messages
pub enum DownlinkRemoteSetup<'a> {
    #[cmd(cid = 0x00, len = 0)]
    PackageVersionReq(PackageVersionReqPayload),
    #[cmd(cid = 0x01, len = 1)]
    McGroupStatusReq(McGroupStatusReqPayload<'a>),
    #[cmd(cid = 0x02, len = 29)]
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
/// Uplink Multicast Remote Setup Messages
pub enum UplinkRemoteSetup<'a> {
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

pub fn parse_downlink_multicast_messages(
    data: &[u8],
) -> MacCommandIterator<'_, DownlinkRemoteSetup<'_>> {
    MacCommandIterator::new(data)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parser::McAddr;

    #[test]
    fn deserialize_commands() {
        let bytes = [
            2, 0, 52, 110, 29, 60, 205, 66, 22, 52, 69, 234, 32, 24, 25, 71, 17, 87, 212, 165, 74,
            142, 0, 0, 0, 0, 255, 255, 255, 255,
        ];
        let mut messages = parse_downlink_multicast_messages(&bytes);
        let first_msg = messages.next().unwrap();
        if let DownlinkRemoteSetup::McGroupSetupReq(mc_group_setup_req) = first_msg {
            assert_eq!(mc_group_setup_req.mc_group_id_header(), 0);
            assert_eq!(mc_group_setup_req.mc_addr(), McAddr::from([52, 110, 29, 60]));
            assert_eq!(
                mc_group_setup_req.mc_key_encrypted(),
                &[205, 66, 22, 52, 69, 234, 32, 24, 25, 71, 17, 87, 212, 165, 74, 142]
            );
            assert_eq!(mc_group_setup_req.min_mc_fcount(), 0);
            assert_eq!(mc_group_setup_req.max_mc_fcount(), 0xFFFFFFFF);
        } else {
            panic!("Should have been a McGroupSetupReq");
        }
    }
}
