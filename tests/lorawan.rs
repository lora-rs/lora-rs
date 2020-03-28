// Copyright (c) 2017,2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

extern crate lorawan;

use heapless;
use heapless::consts::*;

type Vec<T> = heapless::Vec<T,U256>;

use lorawan::creator::*;
use lorawan::keys::*;
use lorawan::maccommandcreator::*;
use lorawan::maccommands::*;
use lorawan::parser::*;

fn phy_join_request_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x00, 0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01, 0x05, 0x04, 0x03, 0x02, 0x05, 0x04,
        0x03, 0x02, 0x2d, 0x10, 0x6a, 0x99, 0x0e, 0x12,
    ]).unwrap();
    res
}

fn phy_join_accept_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x20, 0x49, 0x3e, 0xeb, 0x51, 0xfb, 0xa2, 0x11, 0x6f, 0x81, 0x0e, 0xdb, 0x37, 0x42, 0x97,
        0x51, 0x42,
    ]).unwrap();
    res
}

//fn join_accept_payload_with_c_f_list() -> Vec<u8> {
    //let mut res = Vec::new();
    //res.extend_from_slice(&[
        //0x01, 0x01, 0x01, 0x02, 0x02, 0x02, 0x04, 0x03, 0x02, 0x01, 0x67, 0x09, 0x18, 0x4f, 0x84,
        //0xe8, 0x56, 0x84, 0xb8, 0x5e, 0x84, 0x88, 0x66, 0x84, 0x58, 0x6e, 0x84, 0,
    //]).unwrap();
    //res
    ////867100000, 867300000, 867500000, 867700000, 867900000
//}

fn data_payload() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01, 0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6,
        0xc3, 0xb5, 0x82,
    ]).unwrap();
    res
}

fn data_payload_with_fport_zero() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x40, 0x04, 0x03, 0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x69, 0x36, 0x9e, 0xee, 0x6a, 0xa5,
        0x08,
    ]).unwrap();
    res
}

fn data_payload_with_f_opts() -> Vec<u8> {
    let mut res = Vec::new();
    res.extend_from_slice(&[
        0x40, 0x04, 0x03, 0x02, 0x01, 0x03, 0x00, 0x00, 0x02, 0x03, 0x05, 0xd7, 0xfa, 0x0c, 0x6c
    ]).unwrap();
    res
}

fn app_key() -> [u8; 16] {
    [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ]
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
    let bytes = vec![
        0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x01, 0x02, 0x03, 0x04
    ];
    let phy = parse(bytes);
    assert!(phy.is_err());
}

#[test]
fn test_parse_join_request_payload() {
    let phy = parse(phy_join_request_payload());
    assert_eq!(
        phy,
        Ok(PhyPayload::JoinRequest(PhyJoinRequestPayload::new(phy_join_request_payload()).unwrap()))
    );
}

#[test]
fn test_parse_join_accept_payload() {
    let phy = parse(phy_join_accept_payload());
    assert_eq!(
        phy,
        Ok(PhyPayload::JoinAccept(PhyJoinAcceptPayload::EncryptedJoinAccept(
                    EncryptedPhyJoinAcceptPayload::new(phy_join_accept_payload()).unwrap())))
    );
}

#[test]
fn test_parse_data_payload() {
    let phy = parse(data_payload());
    assert_eq!(
        phy,
        Ok(PhyPayload::DataPayload(PhyDataPayload::EncryptedData(
                    EncryptedPhyDataPayload::new(data_payload()).unwrap())))
    );
}

#[test]
fn test_new_join_accept_payload_too_short() {
    let mut bytes = phy_join_accept_payload();
    let key = AES128(app_key());
    assert!(DecryptedPhyJoinAcceptPayload::new(&mut bytes[1..], &key).is_err());
}

#[test]
fn test_new_join_accept_payload_mic_validation() {
    let decrypted_phy = new_decrypted_join_accept();
    assert_eq!(decrypted_phy.validate_mic(&AES128(app_key())), Ok(true));
}

fn new_decrypted_join_accept() -> DecryptedPhyJoinAcceptPayload<Vec<u8>> {
    let data = phy_join_accept_payload();
    let key = AES128(app_key());
    DecryptedPhyJoinAcceptPayload::new(data, &key).unwrap()
}

#[test]
fn test_new_join_accept_c_f_list_empty() {
    let decrypted_phy = new_decrypted_join_accept();
    assert_eq!(decrypted_phy.c_f_list(), Vec::new());
}

#[test]
fn test_join_accept_app_nonce_extraction() {
    let decrypted_phy = new_decrypted_join_accept();
    // TODO: Check
    let expected = vec![199, 11, 87];
    assert_eq!(decrypted_phy.app_nonce(), AppNonce::new(&expected[..]).unwrap());
}

#[test]
fn test_join_accept_rx_delay_extraction() {
    let decrypted_phy = new_decrypted_join_accept();
    // TODO: Check
    assert_eq!(decrypted_phy.rx_delay(), 0);
}

#[test]
fn test_join_accept_dl_settings_extraction() {
    let decrypted_phy = new_decrypted_join_accept();
    assert_eq!(decrypted_phy.dl_settings(), DLSettings::new(0));
}

#[test]
fn test_dl_settings() {
    let dl_settings = DLSettings::new(0xcb);
    assert_eq!(dl_settings.rx1_dr_offset(), 4);
    assert_eq!(dl_settings.rx2_data_rate(), 11);
}

//#[test]
//fn test_new_join_accept_payload_with_c_f_list() {
    //// TODO: fix
    //let data = join_accept_payload_with_c_f_list();
    //let key = AES128(app_key());
    //let decrypted_phy = DecryptedPhyJoinAcceptPayload::new(data, &key).unwrap();

    //let mut expected_c_f_list = Vec::new();
    //expected_c_f_list.push(Frequency::new_from_raw(&[0x18, 0x4F, 0x84])).unwrap();
    //expected_c_f_list.push(Frequency::new_from_raw(&[0xE8, 0x56, 0x84])).unwrap();
    //expected_c_f_list.push(Frequency::new_from_raw(&[0xB8, 0x5E, 0x84])).unwrap();
    //expected_c_f_list.push(Frequency::new_from_raw(&[0x88, 0x66, 0x84])).unwrap();
    //expected_c_f_list.push(Frequency::new_from_raw(&[0x58, 0x6E, 0x84])).unwrap();
    //assert_eq!(decrypted_phy.c_f_list(), expected_c_f_list);
//}

#[test]
fn test_mic_extraction() {
    let bytes = &data_payload()[..];
    let phy = EncryptedPhyDataPayload::new(bytes);

    assert_eq!(phy.unwrap().mic(), MIC([0xd6, 0xc3, 0xb5, 0x82]));
}

#[test]
fn test_validate_data_mic_when_ok() {
    let phy = EncryptedPhyDataPayload::new(data_payload()).unwrap();
    let key = AES128([2; 16]);

    assert_eq!(phy.validate_mic(&key, 1), Ok(true));
}

#[test]
fn test_validate_data_mic_when_not_ok() {
    let mut bytes = data_payload();
    bytes[8] = 0xee;
    let phy = EncryptedPhyDataPayload::new(bytes).unwrap();
    let key = AES128([2; 16]);

    assert_eq!(phy.validate_mic(&key, 1), Ok(false));
}

#[test]
fn test_new_data_payload_is_none_if_bytes_too_short() {
    let bytes = &[0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x01, 0x02, 0x03, 0x04];
    let bytes_with_fopts =
        &[0x00, 0x04, 0x03, 0x02, 0x01, 0x01, 0xff, 0x04, 0x01, 0x02, 0x03, 0x04];

    assert!(EncryptedPhyDataPayload::new(bytes).is_err());
    assert!(EncryptedPhyDataPayload::new(bytes_with_fopts).is_err());
}

#[test]
fn test_f_port_could_be_absent_in_data_payload() {
    let bytes = &[0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x04, 0x01, 0x02, 0x03, 0x04];
    let data_payload = EncryptedPhyDataPayload::new(bytes).unwrap();
    let mac_payload = data_payload.mac_payload();
    assert!(mac_payload.f_port().is_none());
}

#[test]
fn test_mac_payload_has_good_bytes_when_size_correct() {
    let bytes = &[
        0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0xff, 0x01, 0x02, 0x03, 0x04
    ];
    let phy = EncryptedPhyDataPayload::new(bytes).unwrap();
    let data_payload = phy.mac_payload();
    let expected_bytes = &[0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0xff];
    let expected = DataPayload::new_from_raw(expected_bytes, true);

    assert_eq!(data_payload, expected)
}

#[test]
fn test_complete_data_payload_f_port() {
    let phy = EncryptedPhyDataPayload::new(data_payload()).unwrap();

    assert_eq!(phy.mac_payload().f_port(), Some(1))
}

#[test]
fn test_complete_data_payload_fhdr() {
    let phy = EncryptedPhyDataPayload::new(data_payload()).unwrap();
    let data_payload = phy.mac_payload();
    let fhdr = data_payload.fhdr();

    assert_eq!(fhdr.dev_addr(), DevAddr::new(&[4, 3, 2, 1]).unwrap());

    assert_eq!(fhdr.fcnt(), 1u16);

    let fctrl = fhdr.fctrl();

    assert_eq!(fctrl.f_opts_len(), 0);

    assert!(!fctrl.f_pending(), "no f_pending");

    assert!(!fctrl.ack(), "no ack");

    assert!(fctrl.adr(), "ADR");
}

#[test]
fn test_complete_data_payload_frm_payload() {
    let phy = EncryptedPhyDataPayload::new(data_payload()).unwrap();
    let key = AES128([1; 16]);
    let decrypted = phy.decrypt(None, Some(&key), 1).unwrap();
    let mut payload = Vec::new();
    payload.extend_from_slice(&String::from("hello").into_bytes()[..]).unwrap();

    assert_eq!(decrypted.frm_payload(), Ok(FRMPayload::Data(FRMDataPayload(&payload))));
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
fn test_data_payload_creator() {
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

    assert_eq!(
        phy.build(b"hello", &nwk_skey, &app_skey).unwrap(),
        &data_payload()[..]
    );
}

#[test]
fn test_data_payload_creator_when_payload_and_fport_0() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    phy.set_f_port(0);
    assert!(phy.build(b"hello", &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_fport_0_but_not_encrypt() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    phy.set_f_port(0).set_encrypt_mac_commands(false);
    assert!(phy.build(b"", &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_encrypt_but_not_fport_0() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    let new_channel_req = NewChannelReqPayload::new_as_mac_cmd(&[0x00; 5]).unwrap().0;

    let mut cmds: Vec<&dyn SerializableMacCommand> = Vec::new();
    cmds.extend_from_slice(&[&new_channel_req, &new_channel_req, &new_channel_req]).unwrap();
    phy.set_f_port(1).set_mac_commands(cmds).unwrap();
    assert!(phy.build(b"", &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_big_mac_commands_but_not_fport_0() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    phy.set_f_port(1).set_encrypt_mac_commands(true);
    assert!(phy.build(b"", &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_payload_no_fport() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([2; 16]);
    let app_skey = AES128([1; 16]);
    assert!(phy.build(b"hello", &nwk_skey, &app_skey).is_err());
}

#[test]
fn test_data_payload_creator_when_mac_commands_in_payload() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([1; 16]);
    let mac_cmd1 = MacCommand::LinkCheckReq(LinkCheckReqPayload());
    let mut mac_cmd2 = LinkADRAnsCreator::new();
    mac_cmd2
        .set_channel_mask_ack(true)
        .set_data_rate_ack(false)
        .set_tx_power_ack(true);
    let mut cmds: Vec<&dyn SerializableMacCommand> = Vec::new();
    cmds.extend_from_slice(&[&mac_cmd1, &mac_cmd2]).unwrap();
    phy.set_confirmed(false)
        .set_uplink(true)
        .set_f_port(0)
        .set_dev_addr(&[4, 3, 2, 1])
        .set_fcnt(0)
        .set_mac_commands(cmds).unwrap();
    assert_eq!(
        phy.build(b"", &nwk_skey, &nwk_skey).unwrap(),
        &data_payload_with_fport_zero()[..]
    );
}

#[test]
fn test_data_payload_creator_when_mac_commands_in_f_opts() {
    let mut phy = DataPayloadCreator::new();
    let nwk_skey = AES128([1; 16]);
    let mac_cmd1 = MacCommand::LinkCheckReq(LinkCheckReqPayload());
    let mut mac_cmd2 = LinkADRAnsCreator::new();
    mac_cmd2
        .set_channel_mask_ack(true)
        .set_data_rate_ack(false)
        .set_tx_power_ack(true);
    let mut cmds: Vec<&dyn SerializableMacCommand> = Vec::new();
    cmds.extend_from_slice(&[&mac_cmd1, &mac_cmd2]).unwrap();
    phy.set_confirmed(false)
        .set_uplink(true)
        .set_dev_addr(&[4, 3, 2, 1])
        .set_fcnt(0)
        .set_mac_commands(cmds).unwrap();

    assert_eq!(
        phy.build(b"", &nwk_skey, &nwk_skey).unwrap(),
        &data_payload_with_f_opts()[..]
    );
}
// TODO: test data payload create with piggy_backed mac commands

#[test]
fn test_join_request_dev_eui_extraction() {
    let data = phy_join_request_payload();
    let join_request = PhyJoinRequestPayload::new(&data[..]).unwrap();
    assert_eq!(join_request.dev_eui(), EUI64::new(&data[9..17]).unwrap());
}

#[test]
fn test_join_request_app_eui_extraction() {
    let data = phy_join_request_payload();
    let join_request = PhyJoinRequestPayload::new(&data[..]).unwrap();
    assert_eq!(join_request.app_eui(), EUI64::new(&data[1..9]).unwrap());
}

#[test]
fn test_join_request_dev_nonce_extraction() {
    let data = phy_join_request_payload();
    let join_request = PhyJoinRequestPayload::new(&data[..]).unwrap();
    assert_eq!(
        join_request.dev_nonce(),
        DevNonce::new(&data[17..19]).unwrap()
    );
}

#[test]
fn test_validate_join_request_mic_when_ok() {
    let data = phy_join_request_payload();
    let join_request = PhyJoinRequestPayload::new(&data[..]).unwrap();
    let key = AES128([1; 16]);
    assert_eq!(join_request.validate_mic(&key), Ok(true));
}

#[test]
fn test_validate_join_request_mic_when_not_ok() {
    let data = phy_join_request_payload();
    let join_request = PhyJoinRequestPayload::new(&data[..]).unwrap();
    let key = AES128([2; 16]);
    assert_eq!(join_request.validate_mic(&key), Ok(false));
}

#[test]
#[cfg(feature = "with-downlink")]
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
#[cfg(feature = "with-downlink")]
fn test_join_request_creator() {
    let mut phy = JoinRequestCreator::new();
    let key = AES128([1; 16]);
    phy.set_app_eui(&[0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01])
        .set_dev_eui(&[0x05, 0x04, 0x03, 0x02, 0x05, 0x04, 0x03, 0x02])
        .set_dev_nonce(&[0x2du8, 0x10]);

    assert_eq!(phy.build(&key).unwrap(), &phy_join_request_payload()[..]);
}

#[test]
fn test_derive_newskey(){
    let key = AES128(app_key());
    let join_request = PhyJoinRequestPayload::new(phy_join_request_payload()).unwrap();
    let join_accept = DecryptedPhyJoinAcceptPayload::new(phy_join_accept_payload(), &key).unwrap();

    let newskey = join_accept.derive_newskey(&join_request.dev_nonce(), &key);
    //AppNonce([49, 3e, eb]), NwkAddr([51, fb, a2]), DevNonce([2d, 10])
    let expect = [0x7b, 0xb2, 0x5f, 0x89, 0xe0, 0xd1, 0x37, 0x1e, 0x1f, 0xbf, 0x4d, 0x99, 0x7e,
        0x14, 0x68, 0xa3];
    assert_eq!(newskey.0, expect);
}

#[test]
fn test_derive_appskey(){
    let key = AES128(app_key());
    let join_request = PhyJoinRequestPayload::new(phy_join_request_payload()).unwrap();
    let join_accept = DecryptedPhyJoinAcceptPayload::new(phy_join_accept_payload(), &key).unwrap();

    let appskey = join_accept.derive_appskey(&join_request.dev_nonce(), &key);
    //AppNonce([49, 3e, eb]), NwkAddr([51, fb, a2]), DevNonce([2d, 10])
    let expect = [0x14, 0x88, 0x20, 0xdf, 0xb1, 0xe0, 0xc9, 0xd6, 0x28, 0x9c, 0xde, 0x16, 0xc1,
        0xaf, 0x24, 0x9f];

    assert_eq!(appskey.0, expect);
}

#[test]
#[cfg(feature = "with-to-string")]
fn test_eui64_to_string() {
    let eui = EUI64::new(&[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xff]).unwrap();
    assert_eq!(eui.to_string(), "123456789abcdeff".to_owned());
}
