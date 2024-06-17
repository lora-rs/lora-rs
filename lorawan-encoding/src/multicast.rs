use crate::{
    maccommands::{mac_cmd_zero_len, mac_cmds}
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
        let required_len = McGroupStatusAnsPayload::required_len(status);

        if data.len() < required_len {
            return Err(Error::BufferTooShort);
        }

        Ok(McGroupStatusAnsPayload(&data[0..required_len]))
    }

    pub fn new_from_raw(data: &'a [u8]) -> McGroupStatusAnsPayload<'a> {
        McGroupStatusAnsPayload(data)
    }

    pub fn required_len(status: u8) -> usize {
        // | RFU | NbTotalGroups | AnsGroupMask |
        // |  1  |       3       |      4       |
        let nb_total_groups = (status >> 4) & 0x07; // Extract NbTotalGroups from status
        let required_len = 1 + nb_total_groups as usize * 5; // Each group adds 5 bytes
        required_len
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

pub enum Error {
    BufferTooShort,
    UnknownCommand,
}


pub struct DownlinkMulticastMsgIterator<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> DownlinkMulticastMsgIterator<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        DownlinkMulticastMsgIterator { data, index: 0 }
    }
}

impl<'a> Iterator for DownlinkMulticastMsgIterator<'a> {
    type Item = Result<DownlinkMulticastMsg<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.data.len() {
            return None;
        }

        let cid = self.data[self.index];
        self.index += 1;

        let (msg, len) = match cid {
            0x00 => {
                let len = PackageVersionReqPayload::len();
                let payload = PackageVersionReqPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (DownlinkMulticastMsg::PackageVersionReq(payload), len)
            },
            0x01 => {
                let len = McGroupStatusReqPayload::len();
                let payload = McGroupStatusReqPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (DownlinkMulticastMsg::McGroupStatusReq(payload), len)
            },
            0x02 => {
                let len = McGroupSetupReqPayload::len();
                let payload = McGroupSetupReqPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (DownlinkMulticastMsg::McGroupSetupReq(payload), len)
            },
            0x03 => {
                let len = McGroupDeleteReqPayload::len();
                let payload = McGroupDeleteReqPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (DownlinkMulticastMsg::McGroupDeleteReq(payload), len)
            },
            0x04 => {
                let len = McClassCSessionReqPayload::len();
                let payload = McClassCSessionReqPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (DownlinkMulticastMsg::McClassCSessionReq(payload), len)
            },
            0x05 => {
                let len = McClassBSessionReqPayload::len();
                let payload = McClassBSessionReqPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (DownlinkMulticastMsg::McClassBSessionReq(payload), len)
            },
            _ => return Some(Err(Error::UnknownCommand)),
        };
        self.index += len;
        Some(Ok(msg))
    }
}

pub struct UplinkMulticastMsgIterator<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> UplinkMulticastMsgIterator<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        UplinkMulticastMsgIterator { data, index: 0 }
    }
}

impl<'a> Iterator for UplinkMulticastMsgIterator<'a> {
    type Item = Result<UplinkMulticastMsg<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.data.len() {
            return None;
        }

        let cid = self.data[self.index];
        self.index += 1;

        let (msg, len) = match cid {
            0x00 => {
                let len = PackageVersionAnsPayload::len();
                let payload = PackageVersionAnsPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (UplinkMulticastMsg::PackageVersionAns(payload), len)
            },
            0x01 => {
                // peek at first byte to determine length as this is a variable length message
                let len = McGroupStatusAnsPayload::required_len(self.data[self.index]);
                let payload = McGroupStatusAnsPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (UplinkMulticastMsg::McGroupStatusAns(payload), len)
            },
            0x02 => {
                let len = McGroupSetupAnsPayload::len();
                let payload = McGroupSetupAnsPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (UplinkMulticastMsg::McGroupSetupAns(payload), len)
            },
            0x03 => {
                let len = McGroupDeleteAnsPayload::len();
                let payload = McGroupDeleteAnsPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (UplinkMulticastMsg::McGroupDeleteAns(payload), len)
            },
            0x04 => {
                let len = McClassCSessionAnsPayload::len();
                let payload = McClassCSessionAnsPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (UplinkMulticastMsg::McClassCSessionAns(payload), len)
            },
            0x05 => {
                let len = McClassBSessionAnsPayload::len();
                let payload = McClassBSessionAnsPayload::new_from_raw(&self.data[self.index..self.index + len]);
                (UplinkMulticastMsg::McClassBSessionAns(payload), len)
            },
            _ => return Some(Err(Error::UnknownCommand)),
        };
        self.index += len;
        Some(Ok(msg))
    }
}
