//! Library for parsing and handling LoRaWAN packets.
#![no_std]
#![deny(rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

use crate::maccommands::{DownlinkMacCommand, MacCommandIterator};
use crate::parser::{DevAddr, EncryptedDataPayload, MType, MHDR};

pub mod certification;
pub mod creator;
pub mod keys;
pub mod maccommandcreator;
pub mod maccommands;
pub mod multicast;
pub mod packet_length;
pub mod parser;
pub mod string;
pub mod types;

#[cfg(feature = "full")]
pub mod extra;

#[cfg(feature = "default-crypto")]
#[cfg_attr(docsrs, doc(cfg(feature = "default-crypto")))]
pub mod default_crypto;

mod securityhelpers;

#[test]
fn incorrect_mac_commands() {
    use parser::*;
    // PHYPayload: (MHDR: 60, MACPayload: (FHDR: (DevAddr: 00000000, FCtrl: 00, FCnt: 0010, FOpts: ), FPort: 00, FRMPayload: 03c0000000), MIC: 5e615e64)
    let data = [0x60, 0, 0, 0, 0, 0, 0x10, 0, 0, 3, 0xc0, 0, 0, 0, 0x5e, 0x61, 0x5e, 0x64];
    use crate::extra::std;
    let p = EncryptedDataPayload::new(data).unwrap();

    assert_eq!(p.mhdr(), MHDR::new(0x60));
    assert_eq!(p.f_port(), Some(0));
    assert_eq!(p.fhdr().fcnt(), 0x10);

    assert_eq!(p.mhdr().mtype(), MType::UnconfirmedDataDown);

    let fhdr = p.fhdr();
    assert_eq!(fhdr.dev_addr(), DevAddr::new([0, 0, 0, 0]).unwrap());
    let fopts: std::vec::Vec<_> =
        MacCommandIterator::<DownlinkMacCommand<'_>>::new(fhdr.data()).collect();
    assert_eq!(fopts.len(), 0);

    // TODO: Figure out FRMPayload parsing..
}
