use crate::{
    maccommands::{mac_cmd_zero_len, mac_cmds,
                  Error}
};

pub enum DownlinkMulticastMsg<'a> {
    PackageVersionReq(PackageVersionReqPayload),
    McGroupStatusReq(McGroupStatusReqPayload<'a>),
    McGroupSetupReq(McGroupSetupReqPayload<'a>),
    McGroupDeleteReq(McGroupDeleteReqPayload<'a>),
    McClassCSessionReq(McClassCSessionReqPayload<'a>),
    McClassBSessionReq(McClassBSessionReqPayload<'a>),
}

pub enum UplinkMulticastMsg<'a> {
    PackageVersionAns(PackageVersionAnsPayload<'a>),
    McGroupStatusAns(McGroupStatusAnsPayload<'a>),
    McGroupSetupAns(McGroupSetupAnsPayload<'a>),
    McGroupDeleteAns(McGroupDeleteAnsPayload<'a>),
    McClassCSessionAns(McClassCSessionAnsPayload<'a>),
    McClassBSessionAns(McClassBSessionAnsPayload<'a>),
}

mac_cmd_zero_len! {
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct PackageVersionReqPayload[cmd=PackageVersionReqPayload, cid=0x00, uplink=false]
}

mac_cmds! {
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct PackageVersionAnsPayload[cmd=PackageVersionAns, cid=0x00, uplink=true, size=2]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McGroupStatusReqPayload[cmd=McGroupStatusReq, cid=0x01, uplink=false, size=1]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McGroupSetupReqPayload[cmd=McGroupSetupReq, cid=0x02, uplink=false, size=24]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McGroupSetupAnsPayload[cmd=McGroupSetupAns, cid=0x02, uplink=true, size=1]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McGroupDeleteReqPayload[cmd=McGroupDeleteReq, cid=0x03, uplink=false, size=1]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McGroupDeleteAnsPayload[cmd=McGroupDeleteAns, cid=0x03, uplink=true, size=1]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McClassCSessionReqPayload[cmd=McClassCSessionReq, cid=0x04, uplink=false, size=10]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McClassCSessionAnsPayload[cmd=McClassCSessionAns, cid=0x04, uplink=true, size=4]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McClassBSessionReqPayload[cmd=McClassBSessionReq, cid=0x05, uplink=false, size=10]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct McClassBSessionAnsPayload[cmd=McClassBSessionAns, cid=0x05, uplink=true, size=4]
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq, Eq)]
pub struct McGroupStatusAnsPayload<'a>(pub(crate) &'a [u8]);
impl<'a> McGroupStatusAnsPayload<'a> {
    pub fn new(data: &'a [u8]) -> Result<McGroupStatusAnsPayload<'a>, Error> {
        if data.len() < 1 {
            return Err(Error::BufferTooShort);
        }

        let status = data[0];
        // | RFU | NbTotalGroups | AnsGroupMask |
        // |  1  |       3       |      4       |
        let nb_total_groups = (status >> 4) & 0x07; // Extract NbTotalGroups from status
        let required_len = 1 + nb_total_groups as usize * 5; // Each group adds 5 bytes

        if data.len() < required_len {
            return Err(Error::BufferTooShort);
        }

        Ok(McGroupStatusAnsPayload(&data[0..required_len]))
    }

    pub const fn cid() -> u8 {
        0x01
    }

    pub const fn uplink() -> bool {
        true
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn bytes(&self) -> &[u8] {
        self.0
    }
}