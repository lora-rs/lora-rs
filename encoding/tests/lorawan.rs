// Copyright (c) 2017,2018,2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use lorawan::creator::*;
use lorawan::default_crypto::DefaultFactory;
use lorawan::keys::*;
use lorawan::maccommandcreator::*;
use lorawan::maccommands::*;
use lorawan::parser::*;

fn phy_join_request_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x00, 0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01, 0x05, 0x04, 0x03, 0x02, 0x05, 0x04,
        0x03, 0x02, 0x2d, 0x10, 0x6a, 0x99, 0x0e, 0x12,
    ]);
    res
}

fn phy_join_accept_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x20, 0x49, 0x3e, 0xeb, 0x51, 0xfb, 0xa2, 0x11, 0x6f, 0x81, 0x0e, 0xdb, 0x37, 0x42, 0x97,
        0x51, 0x42,
    ]);
    res
}

fn phy_join_accept_payload_with_c_f_list() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x20, 0xe4, 0x56, 0x73, 0xb6, 0x3c, 0xb4, 0xb9, 0xce, 0xcb, 0x2a, 0xa8, 0x3f, 0x03, 0x33,
        0xe6, 0x15, 0xd2, 0xac, 0x89, 0xee, 0xa1, 0x65, 0x98, 0x37, 0xc3, 0xaa, 0x6d, 0xf9, 0x68,
        0x98, 0x89, 0xcf,
    ]);
    res
    //867100000, 867300000, 867500000, 867700000, 867900000
}

fn phy_dataup_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01, 0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6,
        0xc3, 0xb5, 0x82,
    ]);
    res
}

fn phy_long_dataup_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x40, 0x04, 0x03, 0x02, 0x01, 0x00, 0x00, 0x00, 0x01, 0x27, 0x5a, 0xe9, 0x94, 0x2a, 0x58,
        0x32, 0x21, 0x48, 0xba, 0xd6, 0xca, 0x7d, 0x74, 0x6e, 0x77, 0x4a, 0xf8, 0x66, 0x7a, 0x7b,
        0x72, 0x36, 0x4b, 0xe4, 0xe1, 0x9d, 0x2f, 0x5c, 0x23, 0x98, 0x4f, 0xe2, 0x5e, 0x8e, 0x2d,
        0xdb, 0xd5, 0x15, 0xb5, 0x4e, 0xbe, 0x80, 0xce, 0xc2, 0x1c, 0xd6, 0x5a, 0x88, 0x13, 0x0f,
        0xbe, 0x6d, 0x04, 0xaa, 0xb2, 0xbc, 0x39, 0xab, 0xbe, 0xd9, 0xe8, 0x73, 0xef, 0xc7, 0x85,
        0xe5, 0x65, 0x5d, 0x62, 0x72, 0xf8, 0x79, 0x6b, 0x1e, 0x83, 0x9f, 0x2b, 0x1b, 0xde, 0xab,
        0xa2, 0x01, 0x6c, 0x7e, 0xf9, 0x16, 0x9d, 0x51, 0xf4, 0xea, 0x26, 0x1b, 0xc6, 0x08, 0x9c,
        0x83, 0xb3, 0x3c, 0x6f, 0x30, 0xa7, 0x3c, 0xe1, 0x3c, 0x52, 0x55, 0x7c, 0x46, 0xd7, 0x91,
        0xe7, 0xe0, 0x1b, 0x39, 0xe0, 0xb8, 0x9c, 0x1d, 0x2e, 0x35, 0x08, 0x84, 0x1b, 0x67, 0xe3,
        0xec, 0x88, 0x6f, 0x96, 0xeb, 0x0e, 0x11, 0x16, 0x40, 0xd3, 0xc1, 0x94, 0xf1, 0x21, 0x49,
        0xab, 0x58, 0x4b, 0xd9, 0x31, 0xdc, 0x15, 0xfc, 0x11, 0x94, 0x97, 0xdc, 0xcb, 0xf2, 0xb5,
        0xb9, 0x16, 0xb8, 0x52, 0x42, 0x96, 0x33, 0x41, 0xa5, 0x8b, 0xb5, 0x87, 0x7b, 0xd5, 0xaf,
        0x9e, 0xe4, 0x2d, 0x8b, 0x6f, 0x48, 0x45, 0x85, 0xa6, 0xf9, 0xcb, 0xaf, 0xf7, 0x2e, 0xe1,
        0x09, 0x42, 0xe1, 0x23, 0x8c, 0x98, 0xd7, 0xbf, 0xe7, 0xca, 0x0b, 0x2d, 0xb2, 0x24, 0x8d,
        0xb9, 0x1c, 0xd2, 0x3a, 0x71, 0xc6, 0xdb, 0x9b, 0x76, 0x8c, 0xf7, 0xef, 0x17, 0xf0, 0x51,
        0xcf, 0x42, 0x3e, 0x73, 0x47, 0x7a, 0xbc, 0x9b, 0x0f, 0xf0, 0x62, 0xde, 0x1e, 0x85, 0x20,
        0x29, 0x92, 0xdd, 0xca, 0x58, 0x37, 0x44, 0x19, 0x0c, 0x4f, 0xf7, 0xe1, 0xb4, 0x2e, 0xa3,
        0xcc,
    ]);
    res
}

fn long_data_payload() -> String {
    // some text from loremipsum.de with a typo at the end
    String::from(
        "Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor \
            invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et \
            accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, not",
    )
}

fn phy_datadown_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0xa0, 0x04, 0x03, 0x02, 0x01, 0x80, 0xff, 0x2a, 0x2a, 0x0a, 0xf1, 0xa3, 0x6a, 0x05, 0xd0,
        0x12, 0x5f, 0x88, 0x5d, 0x88, 0x1d, 0x49, 0xe1,
    ]);
    res
}

fn data_payload_with_fport_zero() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x40, 0x04, 0x03, 0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x69, 0x36, 0x9e, 0xee, 0x6a, 0xa5,
        0x08,
    ]);
    res
}

fn data_payload_with_f_opts() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x40, 0x04, 0x03, 0x02, 0x01, 0x03, 0x00, 0x00, 0x02, 0x03, 0x05, 0xd7, 0xfa, 0x0c, 0x6c,
    ]);
    res
}

fn app_key() -> [u8; 16] {
    [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]
}

#[test]
fn test_mhdr_mtype() {
    let examples = [
        (0x00, MType::JoinRequest),
        (0x20, MType::JoinAccept),
        (0x40, MType::UnconfirmedDataUp),
        (0x60, MType::UnconfirmedDataDown),
        (0x80, MType::ConfirmedDataUp),
        (0xa0, MType::ConfirmedDataDown),
        (0xc0, MType::RFU),
        (0xe0, MType::Proprietary),
    ];
    for &(ref v, ref expected) in &examples {
        let mhdr = MHDR::new(*v);
        assert_eq!(mhdr.mtype(), *expected);
    }
}

#[test]
fn test_mhdr_major() {
    let examples = [(0, Major::LoRaWANR1), (1, Major::RFU)];
    for &(ref v, ref expected) in &examples {
        let mhdr = MHDR::new(*v);
        assert_eq!(mhdr.major(), *expected);
    }
}

#[test]
fn test_parse_phy_payload_with_too_few_bytes_is_err() {
    let bytes = vec![0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x01, 0x02, 0x03, 0x04];
    let phy = parse(bytes);
    assert!(phy.is_err());
}

#[test]
fn test_parse_phy_payload_with_unsupported_major_versoin() {
    let bytes = vec![0x81, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x01, 0x02, 0x03, 0x04, 0x05];
    let phy = parse(bytes);

    // this is now part of the API.
    assert_eq!(phy.err(), Some("Unsupported major version"));
}

#[test]
fn test_parse_join_request_payload() {
    let phy = parse(phy_join_request_payload());
    assert_eq!(
        phy,
        Ok(PhyPayload::JoinRequest(JoinRequestPayload::new(phy_join_request_payload()).unwrap()))
    );
}

#[test]
fn test_parse_join_accept_payload() {
    let phy = parse(phy_join_accept_payload());
    assert_eq!(
        phy,
        Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(
            EncryptedJoinAcceptPayload::new(phy_join_accept_payload()).unwrap()
        )))
    );
}

#[test]
fn test_parse_data_payload() {
    let phy = parse(phy_dataup_payload());
    assert_eq!(
        phy,
        Ok(PhyPayload::Data(DataPayload::Encrypted(
            EncryptedDataPayload::new(phy_dataup_payload()).unwrap()
        )))
    );
}

#[test]
fn test_parse_data_payload_no_panic_when_bad_packet() {
    // This reproduces a panic from https://github.com/ivajloip/rust-lorawan/issues/94.
    let data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x85, 0x01, 0x00, 0xd6, 0xc3, 0xb5, 0x82];
    let phy = parse(data);
    assert_eq!(phy.err(), Some("can not build EncryptedDataPayload from the provided data"));
}

#[test]
fn test_parse_data_payload_no_panic_when_too_short_packet() {
    let data = vec![0x40, 0x04, 0x03, 0x02, 0x01];
    let phy = EncryptedDataPayload::new(data);
    assert_eq!(phy.err(), Some("can not build EncryptedDataPayload from the provided data"));
}

#[test]
fn test_new_join_accept_payload_too_short() {
    let mut bytes = phy_join_accept_payload();
    let key = AES128(app_key());
    let len = bytes.len();
    assert!(DecryptedJoinAcceptPayload::new(&mut bytes[..(len - 1)], &key).is_err());
}

#[test]
fn test_new_join_accept_payload_mic_validation() {
    let decrypted_phy = new_decrypted_join_accept();
    assert_eq!(decrypted_phy.validate_mic(&AES128([1; 16])), true);
}

fn new_decrypted_join_accept() -> DecryptedJoinAcceptPayload<Vec<u8>, DefaultFactory> {
    let data = phy_join_accept_payload_with_c_f_list();
    let key = AES128([1; 16]);
    DecryptedJoinAcceptPayload::new(data, &key).unwrap()
}

#[test]
fn test_new_join_accept_c_f_list_empty() {
    let data = phy_join_accept_payload();
    let key = AES128(app_key());
    let decrypted_phy = DecryptedJoinAcceptPayload::new(data, &key).unwrap();
    assert_eq!(decrypted_phy.c_f_list(), None);
}

#[test]
fn test_join_accept_app_nonce_extraction() {
    let decrypted_phy = new_decrypted_join_accept();
    let expected = vec![3, 2, 1];
    assert_eq!(decrypted_phy.app_nonce(), AppNonce::new(&expected[..]).unwrap());
}

#[test]
fn test_join_accept_rx_delay_extraction() {
    let decrypted_phy = new_decrypted_join_accept();
    assert_eq!(decrypted_phy.rx_delay(), 3);
}

#[test]
fn test_join_accept_dl_settings_extraction() {
    let decrypted_phy = new_decrypted_join_accept();
    assert_eq!(decrypted_phy.dl_settings(), DLSettings::new(0x12));
}

#[test]
fn test_dl_settings() {
    let dl_settings = DLSettings::new(0xcb);
    assert_eq!(dl_settings.rx1_dr_offset(), 4);
    assert_eq!(dl_settings.rx2_data_rate(), 11);
}

#[test]
fn test_new_join_accept_payload_with_c_f_list() {
    let data = phy_join_accept_payload_with_c_f_list();
    let key = AES128([1; 16]);
    let decrypted_phy = DecryptedJoinAcceptPayload::new(data, &key).unwrap();

    let expected_c_f_list = CfList::DynamicChannel([
        Frequency::new_from_raw(&[0x18, 0x4F, 0x84]),
        Frequency::new_from_raw(&[0xE8, 0x56, 0x84]),
        Frequency::new_from_raw(&[0xB8, 0x5E, 0x84]),
        Frequency::new_from_raw(&[0x88, 0x66, 0x84]),
        Frequency::new_from_raw(&[0x58, 0x6E, 0x84]),
    ]);
    assert_eq!(decrypted_phy.c_f_list(), Some(expected_c_f_list));
}

#[test]
fn test_mic_extraction() {
    let bytes = &phy_dataup_payload()[..];
    let phy = EncryptedDataPayload::new(bytes);

    assert_eq!(phy.unwrap().mic(), MIC([0xd6, 0xc3, 0xb5, 0x82]));
}

#[test]
fn test_validate_data_mic_when_ok() {
    let phy = EncryptedDataPayload::new(phy_dataup_payload()).unwrap();
    let key = AES128([2; 16]);

    assert_eq!(phy.validate_mic(&key, 1), true);
}

#[test]
fn test_validate_data_mic_when_not_ok() {
    let mut bytes = phy_dataup_payload();
    bytes[8] = 0xee;
    let phy = EncryptedDataPayload::new(bytes).unwrap();
    let key = AES128([2; 16]);

    assert_eq!(phy.validate_mic(&key, 1), false);
}

#[test]
fn test_new_data_payload_is_none_if_bytes_too_short() {
    let bytes = &[0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x01, 0x02, 0x03, 0x04];
    let bytes_with_fopts =
        &[0x00, 0x04, 0x03, 0x02, 0x01, 0x01, 0xff, 0x04, 0x01, 0x02, 0x03, 0x04];

    assert!(EncryptedDataPayload::new(bytes).is_err());
    assert!(EncryptedDataPayload::new(bytes_with_fopts).is_err());
}

#[test]
fn test_f_port_could_be_absent_in_data_payload() {
    let bytes = &[0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x04, 0x01, 0x02, 0x03, 0x04];
    let data_payload = EncryptedDataPayload::new(bytes).unwrap();
    assert!(data_payload.f_port().is_none());
}

#[test]
fn test_complete_data_payload_fhdr() {
    let app_skey = AES128([1; 16]);
    let nwk_skey = AES128([2; 16]);
    let phys: std::vec::Vec<Box<dyn DataHeader>> = vec![
        Box::new(EncryptedDataPayload::new(phy_dataup_payload()).unwrap()),
        Box::new(
            DecryptedDataPayload::new(phy_dataup_payload(), &nwk_skey, Some(&app_skey), 1).unwrap(),
        ),
    ];
    for phy in phys {
        assert_eq!(phy.f_port(), Some(1));

        let fhdr = phy.fhdr();

        assert_eq!(fhdr.dev_addr(), DevAddr::new([4, 3, 2, 1]).unwrap());

        assert_eq!(fhdr.fcnt(), 1u16);

        let fctrl = fhdr.fctrl();

        assert_eq!(fctrl.f_opts_len(), 0);

        assert!(!fctrl.f_pending(), "no f_pending");

        assert!(!fctrl.ack(), "no ack");

        assert!(fctrl.adr(), "ADR");
    }
}

#[test]
fn test_complete_dataup_payload_frm_payload() {
    let phy = EncryptedDataPayload::new(phy_dataup_payload()).unwrap();
    let key = AES128([1; 16]);
    let decrypted = phy.decrypt(None, Some(&key), 1).unwrap();
    let mut payload = Vec::new();
    payload.extend_from_slice(&String::from("hello").into_bytes()[..]);

    assert_eq!(decrypted.frm_payload(), Ok(FRMPayload::Data(&payload)));
}

#[test]
fn test_complete_long_dataup_payload_frm_payload() {
    let phy = EncryptedDataPayload::new(phy_long_dataup_payload()).unwrap();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    let decrypted = phy.decrypt_if_mic_ok(&nwk_skey, &app_skey, 0).unwrap();
    let mut payload = Vec::new();
    payload.extend_from_slice(&long_data_payload().into_bytes()[..]);

    assert_eq!(decrypted.frm_payload(), Ok(FRMPayload::Data(&payload)));
}

#[test]
fn test_complete_datadown_payload_frm_payload() {
    let phy = EncryptedDataPayload::new(phy_datadown_payload()).unwrap();
    let key = AES128([1; 16]);
    let decrypted = phy.decrypt(None, Some(&key), 76543).unwrap();
    let mut payload = Vec::new();
    payload.extend_from_slice(&String::from("hello lora").into_bytes()[..]);

    assert_eq!(decrypted.frm_payload(), Ok(FRMPayload::Data(&payload)));
}

#[test]
fn test_mac_command_in_downlink() {
    let data = [
        0x60, 0x5f, 0x3b, 0xd7, 0x4e, 0x0a, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x70, 0x03, 0x00,
        0xff, 0x00, 0x30, 0xcd, 0xdb, 0x22, 0xee,
    ];
    let packet = EncryptedDataPayload::new(data).unwrap();

    assert_eq!(packet.mhdr().mtype(), MType::UnconfirmedDataDown);

    let fhdr = packet.fhdr();
    assert_eq!(fhdr.fopts().count(), 2);
    for cmd in fhdr.fopts() {
        match cmd {
            MacCommand::LinkADRReq(_) => (),
            _ => panic!("incorrect payload type: {:?}", cmd),
        }
    }
}

#[test]
fn test_decrypt_downlink_missing_f_port_bug() {
    let encrypted_payload = EncryptedDataPayload::new([
        0x60, 0x0, 0x0, 0x0, 0x48, 0xa, 0x0, 0x0, 0x3, 0x0, 0x0, 0x0, 0x70, 0x3, 0x0, 0x0, 0xff,
        0x0, 0xfc, 0x68, 0xf4, 0x5e,
    ])
    .unwrap();
    let key = AES128([1; 16]);
    let fcnt = 0;
    assert!(encrypted_payload.decrypt(Some(&key), None, fcnt as u32).is_ok());
}

#[test]
fn test_new_frequency() {
    let freq = Frequency::new(&[0x18, 0x4F, 0x84]);

    assert!(freq.is_some());
    assert_eq!(freq.unwrap().value(), 867_100_000);
}

#[test]
fn test_fctrl_uplink_complete() {
    let byte = 0xff;
    let uplink_fctrl = FCtrl::new(byte, true);
    assert_eq!(uplink_fctrl.ack(), true);
    assert_eq!(uplink_fctrl.adr(), true);
    assert_eq!(uplink_fctrl.adr_ack_req(), true);
    assert_eq!(uplink_fctrl.f_opts_len(), 15);
    assert_eq!(uplink_fctrl.raw_value(), byte);
}

#[test]
fn test_fctrl_downlink_complete() {
    let downlink_fctrl = FCtrl::new(0xff, false);
    assert_eq!(downlink_fctrl.f_pending(), true);
}

#[test]
fn test_data_payload_uplink_creator() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    let fctrl = FCtrl::new(0x80, true);
    phy.set_confirmed(false)
        .set_uplink(true)
        .set_f_port(1)
        .set_dev_addr(&[4, 3, 2, 1])
        .set_fctrl(&fctrl) // ADR: true, all others: false
        .set_fcnt(1);

    assert_eq!(phy.build(b"hello", &[], &nwk_skey, &app_skey).unwrap(), &phy_dataup_payload()[..]);
}

#[test]
fn test_long_data_payload_uplink_creator() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    let fctrl = FCtrl::new(0x00, true);
    phy.set_confirmed(false)
        .set_uplink(true)
        .set_f_port(1)
        .set_dev_addr(&[4, 3, 2, 1])
        .set_fctrl(&fctrl) // all flags set to false
        .set_fcnt(0);

    assert_eq!(
        phy.build(&long_data_payload().into_bytes()[..], &[], &nwk_skey, &app_skey).unwrap(),
        &phy_long_dataup_payload()[..]
    );
}

#[test]
fn test_data_payload_downlink_creator() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    let fctrl = FCtrl::new(0x80, false);
    phy.set_confirmed(true)
        .set_uplink(false)
        .set_f_port(42)
        .set_dev_addr(&[4, 3, 2, 1])
        .set_fctrl(&fctrl) // ADR: true, all others: false
        .set_fcnt(76543);

    assert_eq!(
        phy.build(b"hello lora", &[], &nwk_skey, &app_skey).unwrap(),
        &phy_datadown_payload()[..]
    );
}

#[test]
fn test_data_payload_creator_when_payload_and_fport_0() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    phy.set_f_port(0);
    assert!(phy.build(b"hello", &[], &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_encrypt_but_not_fport_0() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    let new_channel_req = NewChannelReqPayload::new_as_mac_cmd(&[0x00; 5]).unwrap().0;

    let mut cmds: Vec<&dyn SerializableMacCommand> = Vec::new();
    cmds.extend_from_slice(&[&new_channel_req, &new_channel_req, &new_channel_req]);
    phy.set_f_port(1);
    assert!(phy.build(b"", &cmds[..], &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_payload_no_fport() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    assert!(phy.build(b"hello", &[], &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_mac_commands_in_payload() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([1; 16]);
    let mac_cmd1 = MacCommand::LinkCheckReq(LinkCheckReqPayload());
    let mut mac_cmd2 = LinkADRAnsCreator::new();
    mac_cmd2.set_channel_mask_ack(true).set_data_rate_ack(false).set_tx_power_ack(true);
    let mut cmds: Vec<&dyn SerializableMacCommand> = Vec::new();
    cmds.extend_from_slice(&[&mac_cmd1, &mac_cmd2]);
    phy.set_confirmed(false).set_uplink(true).set_f_port(0).set_dev_addr(&[4, 3, 2, 1]).set_fcnt(0);
    assert_eq!(
        phy.build(b"", &cmds[..], &nwk_skey, &nwk_skey).unwrap(),
        &data_payload_with_fport_zero()[..]
    );
}

#[test]
fn test_data_payload_creator_when_mac_commands_in_f_opts() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([1; 16]);
    let mac_cmd1 = MacCommand::LinkCheckReq(LinkCheckReqPayload());
    let mut mac_cmd2 = LinkADRAnsCreator::new();
    mac_cmd2.set_channel_mask_ack(true).set_data_rate_ack(false).set_tx_power_ack(true);
    let mut cmds: Vec<&dyn SerializableMacCommand> = Vec::new();
    cmds.extend_from_slice(&[&mac_cmd1, &mac_cmd2]);
    phy.set_confirmed(false).set_uplink(true).set_dev_addr(&[4, 3, 2, 1]).set_fcnt(0);

    assert_eq!(
        phy.build(b"", &cmds[..], &nwk_skey, &nwk_skey).unwrap(),
        &data_payload_with_f_opts()[..]
    );
}
// TODO: test data payload create with piggy_backed mac commands

#[test]
fn test_join_request_dev_eui_extraction() {
    let data = phy_join_request_payload();
    let join_request = JoinRequestPayload::new(&data[..]).unwrap();
    assert_eq!(join_request.dev_eui(), EUI64::new(&data[9..17]).unwrap());
}

#[test]
fn test_join_request_app_eui_extraction() {
    let data = phy_join_request_payload();
    let join_request = JoinRequestPayload::new(&data[..]).unwrap();
    assert_eq!(join_request.app_eui(), EUI64::new(&data[1..9]).unwrap());
}

#[test]
fn test_join_request_dev_nonce_extraction() {
    let data = phy_join_request_payload();
    let join_request = JoinRequestPayload::new(&data[..]).unwrap();
    assert_eq!(join_request.dev_nonce(), DevNonce::new(&data[17..19]).unwrap());
}

#[test]
fn test_validate_join_request_mic_when_ok() {
    let data = phy_join_request_payload();
    let join_request = JoinRequestPayload::new(&data[..]).unwrap();
    let key = AES128([1; 16]);
    assert_eq!(join_request.validate_mic(&key), true);
}

#[test]
fn test_validate_join_request_mic_when_not_ok() {
    let data = phy_join_request_payload();
    let join_request = JoinRequestPayload::new(&data[..]).unwrap();
    let key = AES128([2; 16]);
    assert_eq!(join_request.validate_mic(&key), false);
}

#[test]
#[cfg(feature = "default-crypto,with-downlink")]
fn test_join_accept_creator() {
    let mut phy = JoinAcceptCreator::new();
    let key = AES128(app_key());
    let app_nonce_bytes = [0xc7, 0x0b, 0x57];
    phy.set_app_nonce(&app_nonce_bytes)
        .set_net_id(&[0x01, 0x11, 0x22])
        .set_dev_addr(&[0x80, 0x19, 0x03, 0x02])
        .set_dl_settings(0)
        .set_rx_delay(0);

    assert_eq!(phy.build(&key).unwrap(), &phy_join_accept_payload()[..]);
}

#[test]
fn test_join_request_creator() {
    let mut phy = JoinRequestCreator::new();
    let key = AES128([1; 16]);
    phy.set_app_eui(&[0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01])
        .set_dev_eui(&[0x05, 0x04, 0x03, 0x02, 0x05, 0x04, 0x03, 0x02])
        .set_dev_nonce(&[0x2du8, 0x10]);

    assert_eq!(phy.build(&key).unwrap(), &phy_join_request_payload()[..]);
}

#[test]
fn test_join_request_creator_with_options() {
    let mut data = [0; 23];
    {
        let mut phy = JoinRequestCreator::with_options(&mut data[..], DefaultFactory).unwrap();
        let key = AES128([1; 16]);
        phy.set_app_eui(&[0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01])
            .set_dev_eui(&[0x05, 0x04, 0x03, 0x02, 0x05, 0x04, 0x03, 0x02])
            .set_dev_nonce(&[0x2du8, 0x10]);

        assert_eq!(phy.build(&key).unwrap(), &phy_join_request_payload()[..]);
    }
    assert_eq!(&data[..], &phy_join_request_payload()[..]);
}

#[test]
fn test_derive_newskey() {
    let key = AES128(app_key());
    let join_request = JoinRequestPayload::new(phy_join_request_payload()).unwrap();
    let join_accept = DecryptedJoinAcceptPayload::new(phy_join_accept_payload(), &key).unwrap();

    let newskey = join_accept.derive_newskey(&join_request.dev_nonce(), &key);
    //AppNonce([49, 3e, eb]), NwkAddr([51, fb, a2]), DevNonce([2d, 10])
    let expect = [
        0x7b, 0xb2, 0x5f, 0x89, 0xe0, 0xd1, 0x37, 0x1e, 0x1f, 0xbf, 0x4d, 0x99, 0x7e, 0x14, 0x68,
        0xa3,
    ];
    assert_eq!(newskey.0, expect);
}

#[test]
fn test_derive_appskey() {
    let key = AES128(app_key());
    let join_request = JoinRequestPayload::new(phy_join_request_payload()).unwrap();
    let join_accept = DecryptedJoinAcceptPayload::new(phy_join_accept_payload(), &key).unwrap();

    let appskey = join_accept.derive_appskey(&join_request.dev_nonce(), &key);
    //AppNonce([49, 3e, eb]), NwkAddr([51, fb, a2]), DevNonce([2d, 10])
    let expect = [
        0x14, 0x88, 0x20, 0xdf, 0xb1, 0xe0, 0xc9, 0xd6, 0x28, 0x9c, 0xde, 0x16, 0xc1, 0xaf, 0x24,
        0x9f,
    ];

    assert_eq!(appskey.0, expect);
}

#[test]
#[cfg(feature = "with-to-string")]
fn test_eui64_to_string() {
    let eui = EUI64::new(&[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xff]).unwrap();
    assert_eq!(eui.to_string(), "123456789abcdeff".to_owned());
}
