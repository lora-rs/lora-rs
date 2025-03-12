mod group_setup;
mod group_status;
pub use group_status::McGroupStatusAnsCreator;

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

impl PackageVersionAnsCreator {
    /*
    | PackageIdentifier  | PackageVersion |
    |         1          |       1        |
     */
    pub fn package_identifier(&mut self, package_identifier: u8) -> &mut Self {
        self.data[1] = package_identifier;
        self
    }
    pub fn package_version(&mut self, package_version: u8) -> &mut Self {
        self.data[2] = package_version;
        self
    }
}

impl PackageVersionAnsPayload<'_> {
    pub fn package_identifier(&self) -> u8 {
        self.0[0]
    }
    pub fn package_version(&self) -> u8 {
        self.0[1]
    }
}

impl McGroupDeleteReqPayload<'_> {
    pub fn mc_group_id_header(&self) -> u8 {
        self.0[0] & 0b11
    }
}

impl McGroupDeleteReqCreator {
    pub fn mc_group_id_header(&mut self, mc_group_id_header: u8) -> &mut Self {
        const OFFSET: usize = 1;
        self.data[OFFSET] |= mc_group_id_header & 0b11;
        self
    }
}

impl McGroupDeleteAnsPayload<'_> {
    pub fn mc_group_id_header(&self) -> u8 {
        self.0[0] & 0b11
    }
    pub fn mc_group_undefined(&self) -> bool {
        self.0[0] & 0b100 != 0
    }
}

impl McGroupDeleteAnsCreator {
    pub fn mc_group_id_header(&mut self, mc_group_id_header: u8) -> &mut Self {
        self.data[1] &= 0b1111_1100;
        self.data[1] |= mc_group_id_header & 0b11;
        self
    }

    pub fn mc_group_undefined(&mut self, mc_group_undefined: bool) -> &mut Self {
        if mc_group_undefined {
            self.data[1] |= 0b100;
        } else {
            self.data[1] &= 0b1111_1011;
        }
        self
    }
}

pub fn parse_downlink_multicast_messages(
    data: &[u8],
) -> MacCommandIterator<'_, DownlinkRemoteSetup<'_>> {
    MacCommandIterator::new(data)
}

pub fn parse_uplink_multicast_messages(
    data: &[u8],
) -> MacCommandIterator<'_, UplinkRemoteSetup<'_>> {
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

    #[test]
    fn roundtrip_package_version_ans() {
        let mut creator = PackageVersionAnsCreator::new();
        creator.package_identifier(0x01).package_version(0x02);
        let bytes = creator.build();

        let mut messages = parse_uplink_multicast_messages(bytes);
        let msg = messages.next().unwrap();
        if let UplinkRemoteSetup::PackageVersionAns(ans) = msg {
            assert_eq!(ans.package_identifier(), 0x01);
            assert_eq!(ans.package_version(), 0x02);
        } else {
            panic!("Expected PackageVersionAns. Got {msg:?}");
        }
    }

    #[test]
    fn roundtrip_mc_group_delete() {
        let mut creator = McGroupDeleteReqCreator::new();
        creator.mc_group_id_header(3);
        let bytes = creator.build();

        let mut messages = parse_downlink_multicast_messages(bytes);
        let msg = messages.next().unwrap();
        if let DownlinkRemoteSetup::McGroupDeleteReq(req) = msg {
            assert_eq!(req.mc_group_id_header(), 3);
        } else {
            panic!("Expected McGroupDeleteReq. Got {msg:?}");
        }
    }

    #[test]
    fn roundtrip_mc_group_delete_ans_success() {
        let mut creator = McGroupDeleteAnsCreator::new();
        creator.mc_group_id_header(3).mc_group_undefined(false);
        let bytes = creator.build();

        let mut messages = parse_uplink_multicast_messages(bytes);
        let msg = messages.next().unwrap();
        if let UplinkRemoteSetup::McGroupDeleteAns(ans) = msg {
            assert_eq!(ans.mc_group_id_header(), 3);
            assert!(!ans.mc_group_undefined());
        } else {
            panic!("Expected McGroupDeleteAns. Got {msg:?}");
        }
    }

    #[test]
    fn roundtrip_mc_group_delete_ans_failure() {
        let mut creator = McGroupDeleteAnsCreator::new();
        creator.mc_group_id_header(2).mc_group_undefined(true);
        let bytes = creator.build();

        let mut messages = parse_uplink_multicast_messages(bytes);
        let msg = messages.next().unwrap();
        if let UplinkRemoteSetup::McGroupDeleteAns(ans) = msg {
            assert_eq!(ans.mc_group_id_header(), 2);
            assert!(ans.mc_group_undefined());
        } else {
            panic!("Expected McGroupDeleteAns. Got {msg:?}");
        }
    }
}
