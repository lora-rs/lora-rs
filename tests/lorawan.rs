extern crate lorawan;

fn join_request_payload() -> Vec<u8> {
    vec![
        0x00u8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x05u8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x05u8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x2du8,
        0x10u8,
        0x6au8,
        0x99u8,
        0x0eu8,
        0x12,
    ]
}

fn data_payload() -> Vec<u8> {
    vec![
        0x40u8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x80u8,
        0x01u8,
        0x00u8,
        0x01u8,
        0xa6u8,
        0x94u8,
        0x64u8,
        0x26u8,
        0x15u8,
        0xd6u8,
        0xc3u8,
        0xb5u8,
        0x82u8,
    ]
}

#[test]
fn test_mhdr_mtype() {
    let examples = [
        (0 as u8, lorawan::MType::JoinRequest),
        (0x20u8, lorawan::MType::JoinAccept),
        (0x40u8, lorawan::MType::UnconfirmedDataUp),
        (0x60u8, lorawan::MType::UnconfirmedDataDown),
        (0x80u8, lorawan::MType::ConfirmedDataUp),
        (0xa0u8, lorawan::MType::ConfirmedDataDown),
        (0xc0u8, lorawan::MType::RFU),
        (0xe0u8, lorawan::MType::Proprietary),
    ];
    for &(ref v, ref expected) in &examples {
        let mhdr = lorawan::MHDR(*v);
        assert_eq!(mhdr.mtype(), *expected);
    }
}

#[test]
fn test_mhdr_major() {
    let examples = [(0u8, lorawan::Major::LoRaWANR1), (1u8, lorawan::Major::RFU)];
    for &(ref v, ref expected) in &examples {
        let mhdr = lorawan::MHDR(*v);
        assert_eq!(mhdr.major(), *expected);
    }
}

#[test]
fn test_mic() {
    let bytes = &data_payload()[..];
    let phy = lorawan::PhyPayload::new(bytes);

    assert!(phy.is_ok());
    assert_eq!(
        phy.unwrap().mic(),
        lorawan::MIC([0xd6u8, 0xc3u8, 0xb5u8, 0x82u8])
    );
}

#[test]
fn test_phy_payload_is_none_when_too_few_bytes() {
    let bytes = &vec![
        0x80u8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x00u8,
        0xffu8,
        0x01u8,
        0x02u8,
        0x03u8,
        0x04u8,
    ];
    let phy = lorawan::PhyPayload::new(bytes);
    assert!(phy.is_err());
}


#[test]
fn test_new_data_payload_is_none_if_bytes_too_short() {
    let bytes = &[0x04u8, 0x03u8, 0x02u8, 0x01u8, 0x00u8, 0xffu8];
    let bytes_with_fopts = &[0x04u8, 0x03u8, 0x02u8, 0x01u8, 0x01u8, 0xffu8, 0x04u8];

    assert!(lorawan::DataPayload::new(bytes, true).is_none());
    assert!(lorawan::DataPayload::new(bytes_with_fopts, true).is_none());
}

#[test]
fn test_f_port_could_be_absent_in_data_payload() {
    let bytes = &[0x04u8, 0x03u8, 0x02u8, 0x01u8, 0x00u8, 0xffu8, 0x04u8];
    let data_payload = lorawan::DataPayload::new(bytes, true);
    assert!(data_payload.is_some());
    assert!(data_payload.unwrap().f_port().is_none());
}

#[test]
fn test_new_join_accept_payload_is_none_if_bytes_too_short() {}

#[test]
fn test_new_join_accept_payload() {
    let bytes = &[
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x00u8,
        0xffu8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x00u8,
        0xffu8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x00u8,
    ];

    assert!(lorawan::JoinAcceptPayload::new(&bytes[1..]).is_none());
    assert!(lorawan::JoinAcceptPayload::new(bytes).is_some());
}

#[test]
fn test_mac_payload_has_good_bytes_when_size_correct() {
    let bytes = &[
        0x80u8,
        0x04u8,
        0x03u8,
        0x02u8,
        0x01u8,
        0x00u8,
        0xffu8,
        0xffu8,
        0x01u8,
        0x02u8,
        0x03u8,
        0x04u8,
    ];
    let phy_res = lorawan::PhyPayload::new(bytes);
    assert!(phy_res.is_ok());
    let phy = phy_res.unwrap();
    if let lorawan::MacPayload::Data(data_payload) = phy.mac_payload() {
        let expected_bytes = &[0x04u8, 0x03u8, 0x02u8, 0x01u8, 0x00u8, 0xffu8, 0xffu8];
        let expected = lorawan::DataPayload::new(expected_bytes, true).unwrap();

        assert_eq!(data_payload, expected)
    } else {
        panic!("failed to parse DataPayload: {:?}", phy.mac_payload());
    }
}

#[test]
fn test_complete_data_payload_f_port() {
    let data = data_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);

    assert!(phy.is_ok());
    if let lorawan::MacPayload::Data(data_payload) = phy.unwrap().mac_payload() {
        assert_eq!(data_payload.f_port(), Some(1))
    } else {
        panic!("failed to parse DataPayload");
    }
}

#[test]
fn test_complete_data_payload_fhdr() {
    let data = data_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);

    assert!(phy.is_ok());
    if let lorawan::MacPayload::Data(data_payload) = phy.unwrap().mac_payload() {
        let fhdr = data_payload.fhdr();

        assert_eq!(
            fhdr.dev_addr(),
            lorawan::DevAddr::new(&[1u8, 2u8, 3u8, 4u8])
        );

        assert_eq!(fhdr.fcnt(), 1u16);

        let fctrl = fhdr.fctrl();

        assert_eq!(fctrl.f_opts_len(), 0u8);

        assert!(!fctrl.f_pending(), "no f_pending");

        assert!(!fctrl.ack(), "no ack");

        assert!(fctrl.adr(), "ADR");
    } else {
        panic!("failed to parse DataPayload");
    }
}

#[test]
fn test_complete_data_payload_frm_payload() {
    let data = data_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);
    let key = lorawan::AES128([1; 16]);

    assert!(phy.is_ok());
    assert_eq!(
        phy.unwrap().decrypted_payload(&key, 1),
        Ok(lorawan::FRMPayload::Data(
            String::from("hello").into_bytes() as
                lorawan::FRMDataPayload,
        ))
    );
}

#[test]
fn test_validate_data_mic_when_ok() {
    let data = data_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);
    let key = lorawan::AES128([2; 16]);

    assert!(phy.is_ok());
    assert_eq!(phy.unwrap().validate_data_mic(&key, 1), Ok(true));
}

#[test]
fn test_validate_data_mic_when_type_not_ok() {
    let bytes = [0; 23];
    let phy = lorawan::PhyPayload::new(&bytes[..]);
    let key = lorawan::AES128([2; 16]);

    assert!(phy.is_ok());
    assert_eq!(
        phy.unwrap().validate_data_mic(&key, 1),
        Err("Could not read mac payload, maybe of incorrect type")
    );
}

#[test]
fn test_join_request_dev_eui_extraction() {
    let data = join_request_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);

    assert!(phy.is_ok());
    if let lorawan::MacPayload::JoinRequest(join_request) = phy.unwrap().mac_payload() {
        assert_eq!(
            join_request.dev_eui(),
            lorawan::EUI64::new(&data[9..17]).unwrap()
        );
    } else {
        panic!("failed to parse JoinRequest mac payload");
    }
}

#[test]
fn test_join_request_app_eui_extraction() {
    let data = join_request_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);

    assert!(phy.is_ok());
    if let lorawan::MacPayload::JoinRequest(join_request) = phy.unwrap().mac_payload() {
        assert_eq!(
            join_request.app_eui(),
            lorawan::EUI64::new(&data[1..9]).unwrap()
        );
    } else {
        panic!("failed to parse JoinRequest mac payload");
    }
}

#[test]
fn test_join_request_dev_nonce_extraction() {
    let data = join_request_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);

    assert!(phy.is_ok());
    if let lorawan::MacPayload::JoinRequest(join_request) = phy.unwrap().mac_payload() {
        assert_eq!(
            join_request.dev_nonce(),
            lorawan::DevNonce::new(&data[17..19]).unwrap()
        );
    } else {
        panic!("failed to parse JoinRequest mac payload");
    }
}

#[test]
fn test_validate_join_request_mic_when_ok() {
    let data = join_request_payload();
    let phy = lorawan::PhyPayload::new(&data[..]);
    let key = lorawan::AES128([1; 16]);

    assert!(phy.is_ok());
    assert_eq!(phy.unwrap().validate_join_request_mic(&key), Ok(true));
}
