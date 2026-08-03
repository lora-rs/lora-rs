//! Tests for the borrowed-view prototype in `lorawan::v2`.
//!
//! Uses the same wire vectors as tests/lorawan.rs and cross-checks the
//! prototype against the existing `parser` API for byte-identical results.

use lorawan::default_crypto::DefaultFactory;
use lorawan::keys::{AppKey, AppSKey, NwkSKey, AES128, MIC};
use lorawan::v2::*;

use core::str::FromStr;
use std::collections::HashMap;

fn phy_dataup_payload() -> Vec<u8> {
    vec![
        0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01, 0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6,
        0xc3, 0xb5, 0x82,
    ]
}

fn phy_datadown_payload() -> Vec<u8> {
    vec![
        0xa0, 0x04, 0x03, 0x02, 0x01, 0x80, 0xff, 0x2a, 0x2a, 0x0a, 0xf1, 0xa3, 0x6a, 0x05, 0xd0,
        0x12, 0x5f, 0x88, 0x5d, 0x88, 0x1d, 0x49, 0xe1,
    ]
}

fn data_payload_with_fport_zero() -> Vec<u8> {
    vec![
        0x40, 0x04, 0x03, 0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x69, 0x36, 0x9e, 0xee, 0x6a, 0xa5,
        0x08,
    ]
}

fn data_payload_with_f_opts() -> Vec<u8> {
    vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x03, 0x00, 0x00, 0x02, 0x03, 0x05, 0xd7, 0xfa, 0x0c, 0x6c]
}

// ---------------------------------------------------------------------------
// DevAddr: owned newtype ergonomics
// ---------------------------------------------------------------------------

#[test]
fn dev_addr_byte_order_is_explicit() {
    let addr = DevAddr::from_value(0x01020304);
    assert_eq!(addr.as_wire_bytes(), &[0x04, 0x03, 0x02, 0x01]);
    assert_eq!(addr.value(), 0x01020304);
    assert_eq!(addr, DevAddr::from_wire_bytes([0x04, 0x03, 0x02, 0x01]));
    assert_eq!(u32::from(addr), 0x01020304);
}

#[test]
fn dev_addr_display_and_from_str_round_trip() {
    let addr = DevAddr::from_value(0x01020304);
    assert_eq!(addr.to_string(), "01020304");
    assert_eq!(DevAddr::from_str("01020304").unwrap(), addr);
    assert!(DevAddr::from_str("0102030").is_err());
    assert!(DevAddr::from_str("010203045").is_err());
    assert!(DevAddr::from_str("0102030g").is_err());
}

#[test]
fn dev_addr_nwk_id() {
    // NwkID is the top 7 bits of the address value (MSB is wire byte 3).
    let addr = DevAddr::from_value(0xFE00_0000);
    assert_eq!(addr.nwk_id(), 0x7F);
}

#[test]
fn dev_addr_is_a_map_key() {
    // Copy + Eq + Hash: usable directly for session lookup, no to_owned().
    let mut sessions: HashMap<DevAddr, &str> = HashMap::new();
    sessions.insert(DevAddr::from_value(0x01020304), "device-a");

    let buf = phy_dataup_payload();
    let phy = EncryptedDataPayload::parse(&buf).unwrap();
    let addr = phy.fhdr().dev_addr(); // by value, no lifetime attached
    assert_eq!(sessions.get(&addr), Some(&"device-a"));
}

// ---------------------------------------------------------------------------
// Parsing and inspection (read-only view)
// ---------------------------------------------------------------------------

#[test]
fn parse_validates_structure() {
    // Too short
    assert_eq!(EncryptedDataPayload::parse(&[0x40; 11]).unwrap_err(), Error::TooShort);
    // Not a data frame (JoinRequest MHDR)
    let mut join = phy_dataup_payload();
    join[0] = 0x00;
    assert_eq!(EncryptedDataPayload::parse(&join).unwrap_err(), Error::NotADataFrame);
    // Bad major version
    let mut bad_major = phy_dataup_payload();
    bad_major[0] = 0x41;
    assert_eq!(
        EncryptedDataPayload::parse(&bad_major).unwrap_err(),
        Error::UnsupportedMajorVersion
    );
    // FOptsLen overruns the buffer (same vectors as the old test)
    let bytes = [0x80, 0x04, 0x03, 0x02, 0x01, 0x00, 0xff, 0x01, 0x02, 0x03, 0x04];
    assert!(EncryptedDataPayload::parse(&bytes).is_err());
    let bytes = [0x80, 0x04, 0x03, 0x02, 0x01, 0x0f, 0xff, 0x04, 0x01, 0x02, 0x03, 0x04];
    assert_eq!(EncryptedDataPayload::parse(&bytes).unwrap_err(), Error::TruncatedFhdr);
}

#[test]
fn header_accessors_match_old_parser() {
    let buf = phy_dataup_payload();
    let phy = EncryptedDataPayload::parse(&buf).unwrap();

    assert_eq!(phy.frame_type(), DataFrameType::UnconfirmedUp);
    assert!(phy.is_uplink());
    assert!(!phy.is_confirmed());
    assert_eq!(phy.f_port(), Some(1));
    assert_eq!(phy.mic(), MIC([0xd6, 0xc3, 0xb5, 0x82]));

    let fhdr = phy.fhdr();
    assert_eq!(fhdr.dev_addr(), DevAddr::from_value(0x01020304));
    assert_eq!(fhdr.fcnt(), 1);
    assert_eq!(fhdr.f_opts(), &[] as &[u8]);
    assert!(fhdr.fctrl().adr());
    assert!(!fhdr.fctrl().ack());
    assert!(!fhdr.fctrl().f_pending());
    assert_eq!(fhdr.fctrl().f_opts_len(), 0);
}

#[test]
fn f_opts_are_bounds_checked_at_parse_time() {
    let buf = data_payload_with_f_opts();
    let phy = EncryptedDataPayload::parse(&buf).unwrap();
    assert_eq!(phy.fhdr().fctrl().f_opts_len(), 3);
    assert_eq!(phy.fhdr().f_opts(), &[0x02, 0x03, 0x05]);
    assert_eq!(phy.f_port(), None);
}

#[test]
fn f_port_present_with_empty_frm_payload() {
    // MHDR + FHDR(7) + FPort + MIC: FPort 42, no FRMPayload.
    // The old parser reports f_port() == None for this frame.
    let bytes = [0x40, 0x04, 0x03, 0x02, 0x01, 0x00, 0x07, 0x00, 42, 0xde, 0xad, 0xbe, 0xef];
    let phy = EncryptedDataPayload::parse(&bytes).unwrap();
    assert_eq!(phy.f_port(), Some(42));

    let old = lorawan::parser::EncryptedDataPayload::new(bytes).unwrap();
    use lorawan::parser::DataHeader;
    assert_eq!(old.f_port(), None); // documents the old behavior we diverge from
}

#[test]
fn validate_mic_matches_old_parser() {
    let buf = phy_dataup_payload();
    let phy = EncryptedDataPayload::parse(&buf).unwrap();
    let key = AES128([2; 16]);
    assert!(phy.validate_mic(&key, 1, &DefaultFactory));

    let mut tampered = phy_dataup_payload();
    tampered[8] = 0xee;
    let phy = EncryptedDataPayload::parse(&tampered).unwrap();
    assert!(!phy.validate_mic(&key, 1, &DefaultFactory));
}

// ---------------------------------------------------------------------------
// Decryption (typestate through explicit buffer handoff)
// ---------------------------------------------------------------------------

#[test]
fn decrypt_in_place_app_payload() {
    let mut buf = phy_dataup_payload();
    let app_skey = AppSKey::from([1; 16]);
    let dec =
        DecryptedDataPayload::decrypt_in_place(&mut buf, None, Some(&app_skey), 1, &DefaultFactory)
            .unwrap();
    assert_eq!(dec.frm_payload(), FrmPayload::Data(b"hello"));
    // Header accessors still work on the decrypted view.
    assert_eq!(dec.fhdr().dev_addr(), DevAddr::from_value(0x01020304));
    assert_eq!(dec.f_port(), Some(1));
}

#[test]
fn decrypt_in_place_downlink() {
    let mut buf = phy_datadown_payload();
    let app_skey = AppSKey::from([1; 16]);
    let dec = DecryptedDataPayload::decrypt_in_place(
        &mut buf,
        None,
        Some(&app_skey),
        76543,
        &DefaultFactory,
    )
    .unwrap();
    assert_eq!(dec.frm_payload(), FrmPayload::Data(b"hello lora"));
}

#[test]
fn decrypt_fport_zero_uses_nwk_key_and_yields_mac_commands() {
    let mut buf = data_payload_with_fport_zero();
    let nwk_skey = NwkSKey::from([2; 16]);

    // Missing the required key is an error, not a silent wrong decrypt.
    let err =
        DecryptedDataPayload::decrypt_in_place(&mut buf, None, None, 0, &DefaultFactory)
            .unwrap_err();
    assert_eq!(err, Error::MissingKey);

    let dec =
        DecryptedDataPayload::decrypt_in_place(&mut buf, Some(&nwk_skey), None, 0, &DefaultFactory)
            .unwrap();
    match dec.frm_payload() {
        FrmPayload::MacCommands(cmds) => assert!(!cmds.is_empty()),
        other => panic!("expected MAC commands, got {other:?}"),
    }
}

#[test]
fn check_mic_and_decrypt_in_place() {
    let nwk_skey = NwkSKey::from([2; 16]);
    let app_skey = AppSKey::from([1; 16]);

    let mut buf = phy_dataup_payload();
    let dec = DecryptedDataPayload::check_mic_and_decrypt_in_place(
        &mut buf,
        &nwk_skey,
        Some(&app_skey),
        1,
        &DefaultFactory,
    )
    .unwrap();
    assert_eq!(dec.frm_payload(), FrmPayload::Data(b"hello"));

    let mut tampered = phy_dataup_payload();
    tampered[9] ^= 0xff;
    let err = DecryptedDataPayload::check_mic_and_decrypt_in_place(
        &mut tampered,
        &nwk_skey,
        Some(&app_skey),
        1,
        &DefaultFactory,
    )
    .unwrap_err();
    assert_eq!(err, Error::InvalidMic);
}

/// The intended call sequence on a device: inspect the frame read-only to find
/// the session, then hand the buffer over for in-place decryption.
#[test]
fn inspect_then_decrypt_flow() {
    let mut sessions: HashMap<DevAddr, (NwkSKey, AppSKey)> = HashMap::new();
    sessions
        .insert(DevAddr::from_value(0x01020304), (NwkSKey::from([2; 16]), AppSKey::from([1; 16])));

    let mut buf = phy_dataup_payload();

    // Phase 1: read-only borrow to route the frame.
    let (nwk, app) = {
        let phy = EncryptedDataPayload::parse(&buf).unwrap();
        let session = sessions.get(&phy.fhdr().dev_addr()).expect("unknown device");
        assert!(phy.validate_mic(session.0.inner(), 1, &DefaultFactory));
        (session.0, session.1) // keys are Copy
    };

    // Phase 2: mutable handoff, decrypt in place.
    let dec =
        DecryptedDataPayload::decrypt_in_place(&mut buf, Some(&nwk), Some(&app), 1, &DefaultFactory)
            .unwrap();
    assert_eq!(dec.frm_payload(), FrmPayload::Data(b"hello"));
}

// ---------------------------------------------------------------------------
// Equivalence with the old parser
// ---------------------------------------------------------------------------

#[test]
fn decryption_is_byte_identical_to_old_parser() {
    let vectors: [(Vec<u8>, u32); 3] = [
        (phy_dataup_payload(), 1),
        (phy_datadown_payload(), 76543),
        (data_payload_with_fport_zero(), 0),
    ];
    let nwk = NwkSKey::from([2; 16]);
    let app = AppSKey::from([1; 16]);

    for (bytes, fcnt) in vectors {
        // Old API (consumes an owned Vec).
        let old = lorawan::parser::EncryptedDataPayload::new(bytes.clone())
            .unwrap()
            .decrypt(Some(nwk.inner()), Some(app.inner()), fcnt, &DefaultFactory)
            .unwrap();

        // New API (decrypts a caller-owned buffer in place).
        let mut buf = bytes;
        let new = DecryptedDataPayload::decrypt_in_place(
            &mut buf,
            Some(&nwk),
            Some(&app),
            fcnt,
            &DefaultFactory,
        )
        .unwrap();

        use lorawan::parser::AsPhyPayloadBytes;
        assert_eq!(old.as_bytes(), new.as_bytes());
    }
}

// ---------------------------------------------------------------------------
// Top-level parse
// ---------------------------------------------------------------------------

fn phy_join_request_payload() -> Vec<u8> {
    vec![
        0x00, 0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01, 0x05, 0x04, 0x03, 0x02, 0x05, 0x04,
        0x03, 0x02, 0x2d, 0x10, 0x6a, 0x99, 0x0e, 0x12,
    ]
}

fn phy_join_accept_payload() -> Vec<u8> {
    vec![
        0x20, 0x49, 0x3e, 0xeb, 0x51, 0xfb, 0xa2, 0x11, 0x6f, 0x81, 0x0e, 0xdb, 0x37, 0x42, 0x97,
        0x51, 0x42,
    ]
}

fn phy_join_accept_payload_with_c_f_list() -> Vec<u8> {
    vec![
        0x20, 0xe4, 0x56, 0x73, 0xb6, 0x3c, 0xb4, 0xb9, 0xce, 0xcb, 0x2a, 0xa8, 0x3f, 0x03, 0x33,
        0xe6, 0x15, 0xd2, 0xac, 0x89, 0xee, 0xa1, 0x65, 0x98, 0x37, 0xc3, 0xaa, 0x6d, 0xf9, 0x68,
        0x98, 0x89, 0xcf,
    ]
}

fn app_key() -> AppKey {
    AppKey::from([
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ])
}

#[test]
fn parse_classifies_by_mhdr() {
    assert!(matches!(parse(&phy_join_request_payload()), Ok(PhyPayload::JoinRequest(_))));
    assert!(matches!(parse(&phy_join_accept_payload()), Ok(PhyPayload::JoinAccept(_))));
    assert!(matches!(parse(&phy_dataup_payload()), Ok(PhyPayload::Data(_))));
    assert_eq!(parse(&[]).unwrap_err(), Error::TooShort);
    // RFU and Proprietary message types
    assert_eq!(parse(&[0xc0, 0, 0, 0]).unwrap_err(), Error::UnsupportedMessageType);
    assert_eq!(parse(&[0xe0, 0, 0, 0]).unwrap_err(), Error::UnsupportedMessageType);
    // Bad major version
    assert_eq!(parse(&[0x01]).unwrap_err(), Error::UnsupportedMajorVersion);
}

// ---------------------------------------------------------------------------
// JoinRequest
// ---------------------------------------------------------------------------

#[test]
fn join_request_field_extraction() {
    let data = phy_join_request_payload();
    let jr = JoinRequestPayload::parse(&data).unwrap();

    assert_eq!(jr.join_eui(), JoinEui::from_wire_bytes([0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01]));
    assert_eq!(jr.dev_eui(), DevEui::from_wire_bytes([0x05, 0x04, 0x03, 0x02, 0x05, 0x04, 0x03, 0x02]));
    assert_eq!(jr.dev_nonce(), DevNonce::from_wire_bytes([0x2d, 0x10]));
    assert_eq!(jr.dev_nonce().value(), 0x102d);
    assert_eq!(jr.mic(), MIC([0x6a, 0x99, 0x0e, 0x12]));
}

#[test]
fn join_request_mic_validation() {
    let data = phy_join_request_payload();
    let jr = JoinRequestPayload::parse(&data).unwrap();
    assert!(jr.validate_mic(&AppKey::from([1; 16]), &DefaultFactory));
    assert!(!jr.validate_mic(&AppKey::from([2; 16]), &DefaultFactory));
}

#[test]
fn join_request_parse_validates() {
    assert_eq!(JoinRequestPayload::parse(&[]).unwrap_err(), Error::TooShort);
    let short = &phy_join_request_payload()[..22];
    assert_eq!(JoinRequestPayload::parse(short).unwrap_err(), Error::InvalidLength);
    let mut wrong_type = phy_join_request_payload();
    wrong_type[0] = 0x40;
    assert_eq!(JoinRequestPayload::parse(&wrong_type).unwrap_err(), Error::UnexpectedMessageType);
}

/// Round trip through the old creator: build with the old API, parse and
/// validate with the new one.
#[test]
fn join_request_round_trip_through_old_creator() {
    use lorawan::creator::JoinRequestCreator;
    let mut buf = [0u8; 23];
    let mut creator = JoinRequestCreator::new(&mut buf[..]).unwrap();
    let key = AppKey::from([7; 16]);
    creator.set_app_eui(&[1, 2, 3, 4, 5, 6, 7, 8]);
    creator.set_dev_eui(&[8, 7, 6, 5, 4, 3, 2, 1]);
    creator.set_dev_nonce(&[0xcc, 0xdd]);
    let built = creator.build(&key, &DefaultFactory).to_vec();

    let jr = JoinRequestPayload::parse(&built).unwrap();
    assert!(jr.validate_mic(&key, &DefaultFactory));
    assert_eq!(jr.join_eui(), JoinEui::from_wire_bytes([1, 2, 3, 4, 5, 6, 7, 8]));
    assert_eq!(jr.dev_eui(), DevEui::from_wire_bytes([8, 7, 6, 5, 4, 3, 2, 1]));
    assert_eq!(jr.dev_nonce(), DevNonce::from_wire_bytes([0xcc, 0xdd]));
}

// ---------------------------------------------------------------------------
// JoinAccept
// ---------------------------------------------------------------------------

#[test]
fn join_accept_parse_validates() {
    let data = phy_join_accept_payload();
    assert!(EncryptedJoinAcceptPayload::parse(&data).is_ok());
    assert_eq!(
        EncryptedJoinAcceptPayload::parse(&data[..16]).unwrap_err(),
        Error::InvalidLength
    );
    let mut wrong_type = phy_join_accept_payload();
    wrong_type[0] = 0x00;
    assert_eq!(
        EncryptedJoinAcceptPayload::parse(&wrong_type).unwrap_err(),
        Error::UnexpectedMessageType
    );
}

#[test]
fn join_accept_decrypt_and_mic() {
    let mut buf = phy_join_accept_payload_with_c_f_list();
    let key = AppKey::from([1; 16]);
    let ja = DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(
        &mut buf,
        &key,
        &DefaultFactory,
    )
    .unwrap();

    assert_eq!(ja.join_nonce(), JoinNonce::from_wire_bytes([3, 2, 1]));
    assert_eq!(ja.rx_delay(), 3);
    assert_eq!(ja.dl_settings().raw_value(), 0x12);

    let expected = CfList::DynamicChannel([
        Frequency::from_hz(867_100_000),
        Frequency::from_hz(867_300_000),
        Frequency::from_hz(867_500_000),
        Frequency::from_hz(867_700_000),
        Frequency::from_hz(867_900_000),
    ]);
    assert_eq!(ja.c_f_list(), Some(expected));
}

#[test]
fn join_accept_wrong_key_is_invalid_mic() {
    let mut buf = phy_join_accept_payload_with_c_f_list();
    let err = DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(
        &mut buf,
        &AppKey::from([2; 16]),
        &DefaultFactory,
    )
    .unwrap_err();
    assert_eq!(err, Error::InvalidMic);
}

#[test]
fn join_accept_without_c_f_list() {
    let mut buf = phy_join_accept_payload();
    let ja = DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(
        &mut buf,
        &app_key(),
        &DefaultFactory,
    )
    .unwrap();
    assert_eq!(ja.c_f_list(), None);
}

#[test]
fn join_accept_decryption_and_derived_keys_match_old_parser() {
    let key = app_key();
    let dev_nonce = DevNonce::from_wire_bytes([0xcc, 0xdd]);

    // Old API
    let old = lorawan::parser::EncryptedJoinAcceptPayload::new(phy_join_accept_payload())
        .unwrap()
        .decrypt(&key, &DefaultFactory);
    let old_dev_nonce = lorawan::parser::DevNonce::new(&[0xcc, 0xdd][..]).unwrap();
    let old_nwk = old.derive_nwkskey(&old_dev_nonce, &key, &DefaultFactory);
    let old_app = old.derive_appskey(&old_dev_nonce, &key, &DefaultFactory);

    // New API
    let mut buf = phy_join_accept_payload();
    let ja =
        DecryptedJoinAcceptPayload::decrypt_in_place(&mut buf, &key, &DefaultFactory).unwrap();

    use lorawan::parser::AsPhyPayloadBytes;
    assert_eq!(old.as_bytes(), ja.as_bytes());
    assert_eq!(old_nwk, ja.derive_nwkskey(dev_nonce, &key, &DefaultFactory));
    assert_eq!(old_app, ja.derive_appskey(dev_nonce, &key, &DefaultFactory));
}

// ---------------------------------------------------------------------------
// MAC commands: Result-yielding iterator
// ---------------------------------------------------------------------------

mod mac_commands {
    use lorawan::maccommands::{DownlinkMacCommand, UplinkMacCommand};
    use lorawan::v2::mac::{
        parse_downlink_mac_commands, parse_uplink_mac_commands, Error as MacError,
    };

    #[test]
    fn parses_valid_uplink_stream() {
        // LinkCheckReq (0 bytes) + LinkADRAns (1 byte)
        let data = [0x02, 0x03, 0x05];
        let cmds: Vec<_> = parse_uplink_mac_commands(&data).collect::<Result<_, _>>().unwrap();
        assert_eq!(cmds.len(), 2);
        assert!(matches!(cmds[0], UplinkMacCommand::LinkCheckReq(_)));
        match &cmds[1] {
            UplinkMacCommand::LinkADRAns(ans) => {
                assert!(ans.channel_mask_ack());
                assert!(!ans.data_rate_ack());
                assert!(ans.powert_ack());
            }
            other => panic!("expected LinkADRAns, got {other:?}"),
        }
    }

    #[test]
    fn parses_valid_downlink_stream() {
        // Two LinkADRReq commands (fopts from the old mac_command_in_fopts test)
        let data = [0x03, 0x00, 0x00, 0x00, 0x70, 0x03, 0x00, 0xff, 0x00, 0x30];
        let cmds: Vec<_> = parse_downlink_mac_commands(&data).collect::<Result<_, _>>().unwrap();
        assert_eq!(cmds.len(), 2);
        assert!(cmds.iter().all(|c| matches!(c, DownlinkMacCommand::LinkADRReq(_))));
    }

    #[test]
    fn unknown_cid_yields_error_then_fuses() {
        let data = [0x02, 0xff, 0x03, 0x05];
        let mut iter = parse_uplink_mac_commands(&data);
        assert!(matches!(iter.next(), Some(Ok(UplinkMacCommand::LinkCheckReq(_)))));
        assert_eq!(iter.next(), Some(Err(MacError::UnknownCid(0xff))));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn truncated_payload_yields_error() {
        // LinkADRReq needs 4 payload bytes, only 1 present
        let data = [0x03, 0x00];
        let mut iter = parse_downlink_mac_commands(&data);
        assert_eq!(iter.next(), Some(Err(MacError::Truncated { cid: 0x03 })));
        assert_eq!(iter.next(), None);
    }

    /// The old iterator ends silently on a malformed tail; the new one
    /// reports it. This documents the divergence.
    #[test]
    fn old_iterator_swallows_malformed_tail() {
        let data = [0x02, 0xff, 0x03, 0x05];
        let old: Vec<_> =
            lorawan::maccommands::parse_uplink_mac_commands(&data).collect();
        assert_eq!(old.len(), 1); // garbage tail indistinguishable from clean end

        let new: Vec<_> = parse_uplink_mac_commands(&data).collect();
        assert_eq!(new.len(), 2); // last item is the error
        assert!(new[1].is_err());
    }

    #[test]
    fn equivalent_to_old_iterator_on_valid_input() {
        let vectors: &[&[u8]] = &[
            &[0x02, 0x03, 0x05],
            &[0x06, 0x02, 0x0a, 0x07, 0x01, 0x08, 0x09],
            &[],
        ];
        for data in vectors {
            let old: Vec<_> = lorawan::maccommands::parse_uplink_mac_commands(data).collect();
            let new: Vec<_> =
                parse_uplink_mac_commands(data).collect::<Result<_, _>>().unwrap();
            assert_eq!(old, new);
        }
    }
}

// ---------------------------------------------------------------------------
// Creators: struct-literal frame description, build_into a caller buffer
// ---------------------------------------------------------------------------

mod creators {
    use super::*;
    use core::num::NonZeroU8;

    #[test]
    fn join_request_matches_old_creator() {
        let key = AppKey::from([7; 16]);
        let jr = JoinRequest {
            join_eui: JoinEui::from_wire_bytes([1; 8]),
            dev_eui: DevEui::from_wire_bytes([2; 8]),
            dev_nonce: DevNonce::from_wire_bytes([3, 3]),
        };
        let mut buf = [0u8; 23];
        let built = jr.build_into(&mut buf, &key, &DefaultFactory).unwrap().to_vec();

        let mut old_buf = [0u8; 100];
        let mut old = lorawan::creator::JoinRequestCreator::new(&mut old_buf[..]).unwrap();
        old.set_app_eui(&[1; 8]);
        old.set_dev_eui(&[2; 8]);
        old.set_dev_nonce(&[3; 2]);
        let old_built = old.build(&key, &DefaultFactory);

        assert_eq!(built, old_built);

        // And the new parser accepts and MIC-validates it.
        let parsed = JoinRequestPayload::parse(&built).unwrap();
        assert!(parsed.validate_mic(&key, &DefaultFactory));
    }

    #[test]
    fn join_accept_matches_old_creator_and_round_trips() {
        let key = AppKey::from([1; 16]);
        let ja = JoinAccept {
            join_nonce: JoinNonce::from_wire_bytes([1, 1, 1]),
            net_id: NetId::from_wire_bytes([1, 1, 1]),
            dev_addr: DevAddr::from_wire_bytes([1, 1, 1, 1]),
            dl_settings: lorawan::types::DLSettings::new(2),
            rx_delay: 1,
            c_f_list: Some(CfList::DynamicChannel([
                Frequency::from_hz(867_100_000),
                Frequency::from_hz(867_300_000),
                Frequency::from_hz(867_500_000),
                Frequency::from_hz(867_700_000),
                Frequency::from_hz(867_900_000),
            ])),
        };
        let mut buf = [0u8; 33];
        let built = ja.build_into(&mut buf, &key, &DefaultFactory).unwrap().to_vec();

        // Old creator with identical inputs.
        let mut old_buf = [0u8; 33];
        let mut old = lorawan::creator::JoinAcceptCreator::new(&mut old_buf[..]).unwrap();
        old.set_app_nonce(&[1u8, 1, 1]);
        old.set_net_id(&[1u8, 1, 1]);
        old.set_dev_addr(&[1u8, 1, 1, 1]);
        old.set_dl_settings(2);
        old.set_rx_delay(1);
        let freqs = [
            lorawan::types::Frequency::new(&[0x18, 0x4F, 0x84]).unwrap(),
            lorawan::types::Frequency::new(&[0xE8, 0x56, 0x84]).unwrap(),
            lorawan::types::Frequency::new(&[0xB8, 0x5E, 0x84]).unwrap(),
            lorawan::types::Frequency::new(&[0x88, 0x66, 0x84]).unwrap(),
            lorawan::types::Frequency::new(&[0x58, 0x6E, 0x84]).unwrap(),
        ];
        old.set_c_f_list(&freqs[..]).unwrap();
        let old_built = old.build(key.inner(), &DefaultFactory).unwrap();
        assert_eq!(built, old_built);

        // Round trip through the new parser.
        let mut rt = built.clone();
        let parsed = DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(
            &mut rt,
            &key,
            &DefaultFactory,
        )
        .unwrap();
        assert_eq!(parsed.join_nonce(), ja.join_nonce);
        assert_eq!(parsed.net_id(), ja.net_id);
        assert_eq!(parsed.dev_addr(), ja.dev_addr);
        assert_eq!(parsed.rx_delay(), 1);
        assert_eq!(parsed.c_f_list(), ja.c_f_list);
    }

    /// Building the uplink test vector from scratch reproduces it exactly,
    /// MIC included.
    #[test]
    fn data_frame_reproduces_uplink_vector() {
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedUp,
            dev_addr: DevAddr::from_value(0x01020304),
            adr: true,
            fcnt: 1,
            payload: Payload::Data { f_port: NonZeroU8::new(1).unwrap(), data: b"hello" },
            ..Default::default()
        };
        let mut buf = [0u8; 64];
        let built = frame
            .build_into(
                &mut buf,
                &NwkSKey::from([2; 16]),
                Some(&AppSKey::from([1; 16])),
                &DefaultFactory,
            )
            .unwrap();
        assert_eq!(built, &phy_dataup_payload()[..]);
    }

    /// Same for the confirmed downlink vector (32-bit fcnt: only the low 16
    /// bits are on the wire).
    #[test]
    fn data_frame_reproduces_downlink_vector() {
        let frame = DataFrame {
            frame_type: DataFrameType::ConfirmedDown,
            dev_addr: DevAddr::from_value(0x01020304),
            adr: true,
            fcnt: 76543,
            payload: Payload::Data { f_port: NonZeroU8::new(42).unwrap(), data: b"hello lora" },
            ..Default::default()
        };
        let mut buf = [0u8; 64];
        let built = frame
            .build_into(
                &mut buf,
                &NwkSKey::from([2; 16]),
                Some(&AppSKey::from([1; 16])),
                &DefaultFactory,
            )
            .unwrap();
        assert_eq!(built, &phy_datadown_payload()[..]);
    }

    #[test]
    fn data_frame_round_trips_mac_commands_vector() {
        // Decrypt the fport-0 vector with the old API to recover the MAC
        // command plaintext, rebuild it with the new creator, and compare.
        let nwk = NwkSKey::from([1; 16]);
        let mut plain = data_payload_with_fport_zero();
        let dec = DecryptedDataPayload::decrypt_in_place(
            &mut plain,
            Some(&nwk),
            None,
            0,
            &DefaultFactory,
        )
        .unwrap();
        let FrmPayload::MacCommands(cmds) = dec.frm_payload() else {
            panic!("expected MAC commands");
        };
        let cmds = cmds.to_vec();

        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedUp,
            dev_addr: DevAddr::from_value(0x01020304),
            fcnt: 0,
            payload: Payload::MacCommands(&cmds),
            ..Default::default()
        };
        let mut buf = [0u8; 64];
        let built = frame.build_into(&mut buf, &nwk, None, &DefaultFactory).unwrap();
        assert_eq!(built, &data_payload_with_fport_zero()[..]);
    }

    #[test]
    fn data_frame_reproduces_f_opts_vector() {
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedUp,
            dev_addr: DevAddr::from_value(0x01020304),
            fcnt: 0,
            f_opts: &[0x02, 0x03, 0x05],
            payload: Payload::None,
            ..Default::default()
        };
        let mut buf = [0u8; 64];
        let built = frame
            .build_into(&mut buf, &NwkSKey::from([1; 16]), None, &DefaultFactory)
            .unwrap();
        assert_eq!(built, &data_payload_with_f_opts()[..]);
    }

    #[test]
    fn data_frame_build_validation() {
        let nwk = NwkSKey::from([2; 16]);
        let app = AppSKey::from([1; 16]);
        let base = DataFrame { dev_addr: DevAddr::from_value(1), ..Default::default() };
        let mut buf = [0u8; 64];

        // FOpts limited to 15 bytes.
        let frame = DataFrame { f_opts: &[0; 16], ..base };
        assert_eq!(
            frame.build_into(&mut buf, &nwk, None, &DefaultFactory).unwrap_err(),
            Error::FOptsTooLong
        );

        // FPort 0 with non-empty FOpts is forbidden.
        let frame =
            DataFrame { f_opts: &[0x02], payload: Payload::MacCommands(&[0x02]), ..base };
        assert_eq!(
            frame.build_into(&mut buf, &nwk, None, &DefaultFactory).unwrap_err(),
            Error::FOptsWithFPortZero
        );

        // App data needs the AppSKey.
        let frame = DataFrame {
            payload: Payload::Data { f_port: NonZeroU8::new(1).unwrap(), data: b"x" },
            ..base
        };
        assert_eq!(
            frame.build_into(&mut buf, &nwk, None, &DefaultFactory).unwrap_err(),
            Error::MissingKey
        );

        // Output buffer too small.
        let mut small = [0u8; 12];
        assert_eq!(
            frame
                .build_into(&mut small, &nwk, Some(&app), &DefaultFactory)
                .unwrap_err(),
            Error::BufferTooShort
        );
    }

    /// Full new-API round trip: build, parse, MIC-check, decrypt, compare.
    #[test]
    fn build_parse_decrypt_round_trip() {
        let nwk = NwkSKey::from([3; 16]);
        let app = AppSKey::from([4; 16]);
        let frame = DataFrame {
            frame_type: DataFrameType::ConfirmedUp,
            dev_addr: DevAddr::from_value(0xdeadbeef),
            adr: true,
            ack: true,
            fcnt: 0x0004_0007,
            f_opts: &[0x02], // LinkCheckReq
            payload: Payload::Data { f_port: NonZeroU8::new(7).unwrap(), data: b"payload" },
            ..Default::default()
        };
        let mut buf = [0u8; 64];
        let built_len =
            frame.build_into(&mut buf, &nwk, Some(&app), &DefaultFactory).unwrap().len();

        let mut rt = buf[..built_len].to_vec();
        let dec = DecryptedDataPayload::check_mic_and_decrypt_in_place(
            &mut rt,
            &nwk,
            Some(&app),
            0x0004_0007,
            &DefaultFactory,
        )
        .unwrap();
        assert_eq!(dec.frame_type(), DataFrameType::ConfirmedUp);
        assert_eq!(dec.fhdr().dev_addr(), DevAddr::from_value(0xdeadbeef));
        assert_eq!(dec.fhdr().fcnt(), 0x0007);
        assert_eq!(dec.fhdr().f_opts(), &[0x02]);
        assert!(dec.fhdr().fctrl().adr());
        assert!(dec.fhdr().fctrl().ack());
        assert_eq!(dec.f_port(), Some(7));
        assert_eq!(dec.frm_payload(), FrmPayload::Data(b"payload"));
    }
}

// ---------------------------------------------------------------------------
// DevEui / JoinEui distinct newtypes
// ---------------------------------------------------------------------------

mod euis {
    use super::*;

    /// Same LSB-wire / MSB-display conventions as keys::DevEui.
    #[test]
    fn dev_eui_conventions_match_old_type() {
        let wire = [0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12];
        let new = DevEui::from_wire_bytes(wire);
        let old = lorawan::keys::DevEui::from(wire);
        assert_eq!(new.to_string(), old.to_string());
        assert_eq!(new.to_string(), "123456789abcdef0");
        assert_eq!(
            DevEui::from_str("123456789abcdef0").unwrap(),
            lorawan::keys::DevEui::from_str("123456789abcdef0").unwrap().into(),
        );
        assert_eq!(new.value(), 0x123456789abcdef0);
        // Round trip through the old type.
        assert_eq!(DevEui::from(lorawan::keys::DevEui::from(new)), new);
    }

    /// join_eui and dev_eui are now different types; mixing them up is a
    /// compile error rather than a swapped-EUI bug on air.
    #[test]
    fn join_request_uses_distinct_types() {
        let jr = JoinRequest {
            join_eui: JoinEui::from_value(0x0101010101010101),
            dev_eui: DevEui::from_value(0x0202020202020202),
            dev_nonce: DevNonce::from_value(1),
        };
        // jr.join_eui = jr.dev_eui; // does not compile
        assert_eq!(jr.join_eui.value(), 0x0101010101010101);
    }
}

// ---------------------------------------------------------------------------
// Certification (TS009) through v2 framing
// ---------------------------------------------------------------------------

mod certification {
    use lorawan::certification::*;
    use lorawan::v2::certification::{
        parse_downlink_dut_commands, parse_uplink_dut_commands, DownlinkDUTCommand,
        UplinkDUTCommand,
    };
    use lorawan::v2::mac::Error as MacError;

    #[test]
    fn parses_fixed_and_variable_commands() {
        // DutResetReq (0) + TxPeriodicityChangeReq (1 byte) + EchoIncPayloadReq
        // (variable: consumes the rest).
        let data = [0x01, 0x06, 0x03, 0x08, 0xaa, 0xbb];
        let cmds: Vec<_> =
            parse_downlink_dut_commands(&data).collect::<Result<_, _>>().unwrap();
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[0], DownlinkDUTCommand::DutResetReq(_)));
        match &cmds[1] {
            DownlinkDUTCommand::TxPeriodicityChangeReq(p) => {
                assert_eq!(p.periodicity(), Ok(Some(20)));
            }
            other => panic!("expected TxPeriodicityChangeReq, got {other:?}"),
        }
        match &cmds[2] {
            DownlinkDUTCommand::EchoIncPayloadReq(p) => assert_eq!(p.payload(), &[0xaa, 0xbb]),
            other => panic!("expected EchoIncPayloadReq, got {other:?}"),
        }
    }

    #[test]
    fn echo_round_trip_through_old_creator() {
        let mut creator = EchoIncPayloadAnsCreator::new();
        creator.payload(&[1, 2, 3]);
        let bytes = creator.build().to_vec();

        let cmds: Vec<_> =
            parse_uplink_dut_commands(&bytes).collect::<Result<_, _>>().unwrap();
        match &cmds[..] {
            [UplinkDUTCommand::EchoIncPayloadAns(p)] => assert_eq!(p.payload(), &[2, 3, 4]),
            other => panic!("expected EchoIncPayloadAns, got {other:?}"),
        }
    }

    #[test]
    fn dut_versions_ans_round_trip() {
        let mut creator = DutVersionsAnsCreator::new();
        creator.set_versions_raw([1, 0, 0, 0, 1, 0, 4, 0, 1, 0, 0, 1]);
        let bytes = creator.build().to_vec();
        let cmds: Vec<_> =
            parse_uplink_dut_commands(&bytes).collect::<Result<_, _>>().unwrap();
        assert!(matches!(cmds[..], [UplinkDUTCommand::DutVersionsAns(_)]));
    }

    /// A lone variable-length CID is Truncated in v2. The old iterator
    /// fabricates an empty EchoIncPayloadAns command from it.
    #[test]
    fn lone_variable_cid_is_truncated() {
        let data = [0x08];
        let mut iter = parse_uplink_dut_commands(&data);
        assert_eq!(iter.next(), Some(Err(MacError::Truncated { cid: 0x08 })));
        assert_eq!(iter.next(), None);

        // Old behavior, documented: yields a command with an empty payload.
        let old: Vec<_> =
            lorawan::maccommands::MacCommandIterator::<UplinkDUTCommand>::new(&data).collect();
        assert_eq!(old.len(), 1);
    }

    #[test]
    fn unknown_cid_is_reported() {
        let data = [0x01, 0x55];
        let mut iter = parse_downlink_dut_commands(&data);
        assert!(matches!(iter.next(), Some(Ok(DownlinkDUTCommand::DutResetReq(_)))));
        assert_eq!(iter.next(), Some(Err(MacError::UnknownCid(0x55))));
        assert_eq!(iter.next(), None);
    }
}

// ---------------------------------------------------------------------------
// Multicast (TS005) through v2 framing
// ---------------------------------------------------------------------------

mod multicast {
    use lorawan::default_crypto::DefaultFactory;
    use lorawan::keys::{McKEKey, McKey};
    use lorawan::multicast::{McGroupSetupReqCreator, McGroupStatusAnsCreator};
    use lorawan::parser::McAddr;
    use lorawan::v2::mac::Error as MacError;
    use lorawan::v2::multicast::{
        parse_downlink_multicast_commands, parse_uplink_multicast_commands, DownlinkRemoteSetup,
        UplinkRemoteSetup,
    };

    #[test]
    fn parses_group_setup_req() {
        // Vector from the old deserialize_commands test.
        let bytes = [
            2, 0, 52, 110, 29, 60, 205, 66, 22, 52, 69, 234, 32, 24, 25, 71, 17, 87, 212, 165,
            74, 142, 0, 0, 0, 0, 255, 255, 255, 255,
        ];
        let cmds: Vec<_> =
            parse_downlink_multicast_commands(&bytes).collect::<Result<_, _>>().unwrap();
        match &cmds[..] {
            [DownlinkRemoteSetup::McGroupSetupReq(req)] => {
                assert_eq!(req.mc_group_id_header(), 0);
                assert_eq!(req.mc_addr(), McAddr::from([52, 110, 29, 60]));
                assert_eq!(req.min_mc_fcount(), 0);
                assert_eq!(req.max_mc_fcount(), 0xFFFFFFFF);
            }
            other => panic!("expected McGroupSetupReq, got {other:?}"),
        }
    }

    /// Session derivation works through v2 framing (payload types shared).
    #[test]
    fn derive_session_through_v2_framing() {
        let mc_addr = McAddr::from([52, 110, 29, 60]);
        let mc_key = McKey::from([0x44; 16]);
        let mcke_key = McKEKey::from([0x66; 16]);
        let mut creator = McGroupSetupReqCreator::new();
        creator.mc_group_id_header(0x01);
        creator.mc_addr(&mc_addr);
        creator.mc_key(&DefaultFactory, &mc_key, &mcke_key);
        creator.min_mc_fcount(7);
        creator.max_mc_fcount(1000);
        let bytes = creator.build().to_vec();

        let cmds: Vec<_> =
            parse_downlink_multicast_commands(&bytes).collect::<Result<_, _>>().unwrap();
        let [DownlinkRemoteSetup::McGroupSetupReq(req)] = &cmds[..] else {
            panic!("expected McGroupSetupReq");
        };
        let (group_id, session) = req.derive_session(&DefaultFactory, &mcke_key);
        assert_eq!(group_id, 1);
        assert_eq!(session.multicast_addr(), mc_addr);
        assert_eq!(session.fcnt_down, 7);
        assert_eq!(session.max_fcnt_down(), 1000);
        // Keys must match direct derivation from McKey.
        assert_eq!(
            session.mc_app_s_key(),
            mc_key.derive_mc_app_s_key(&DefaultFactory, &mc_addr)
        );
        assert_eq!(
            session.mc_net_s_key(),
            mc_key.derive_mc_net_s_key(&DefaultFactory, &mc_addr)
        );
    }

    /// McGroupStatusAns is variable length, driven by a bitmask in its first
    /// byte; v2 framing consumes exactly the right number of bytes.
    #[test]
    fn group_status_ans_variable_length() {
        let mut creator = McGroupStatusAnsCreator::new();
        creator.nb_total_groups(2);
        creator.push(0, McAddr::from([1, 2, 3, 4])).unwrap();
        creator.push(2, McAddr::from([5, 6, 7, 8])).unwrap();
        let mut bytes = creator.build().to_vec();
        // Append a second command to prove framing consumes exactly one.
        bytes.push(0x00); // PackageVersionReq? (downlink) -- uplink 0x00 is PackageVersionAns len 2
        bytes.extend_from_slice(&[0x07, 0x01]);

        let cmds: Vec<_> =
            parse_uplink_multicast_commands(&bytes).collect::<Result<_, _>>().unwrap();
        assert_eq!(cmds.len(), 2);
        match &cmds[0] {
            UplinkRemoteSetup::McGroupStatusAns(ans) => {
                assert_eq!(ans.nb_total_groups(), 2);
                assert_eq!(ans.ans_group_mask(), 0b101);
                let items: Vec<_> = ans.item_iterator().collect();
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].mc_group_id(), 0);
                assert_eq!(items[1].mc_addr().as_ref(), &[5, 6, 7, 8]);
            }
            other => panic!("expected McGroupStatusAns, got {other:?}"),
        }
        assert!(matches!(cmds[1], UplinkRemoteSetup::PackageVersionAns(_)));
    }

    /// A lone McGroupStatusAns CID byte PANICS in the old iterator (its
    /// length callback indexes an empty slice). v2 reports Truncated.
    #[test]
    fn lone_group_status_cid_panics_old_but_errors_new() {
        let data = [0x01];

        let mut iter = parse_uplink_multicast_commands(&data);
        assert_eq!(iter.next(), Some(Err(MacError::Truncated { cid: 0x01 })));

        // Document the old panic (remotely-reachable from frame contents).
        let old = std::panic::catch_unwind(|| {
            lorawan::multicast::parse_uplink_multicast_messages(&data).next()
        });
        assert!(old.is_err(), "old iterator was expected to panic on this input");
    }

    /// Truncated McGroupStatusAns items (mask promises more than present).
    #[test]
    fn group_status_ans_truncated_items() {
        // status: mask 0b11 (2 groups) but only one 5-byte item present
        let data = [0x01, 0x23, 0, 1, 2, 3, 4];
        let mut iter = parse_uplink_multicast_commands(&data);
        assert_eq!(iter.next(), Some(Err(MacError::Truncated { cid: 0x01 })));
    }
}
