use super::*;
use crate::async_device::McAddr;
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::default_crypto::DefaultNetworkCrypto;
use lorawan::keys::{McKEKey, McKey};
use lorawan::multicast::parse_uplink_multicast_commands;
use lorawan::multicast::{McGroupDeleteReqCreator, McGroupSetupReqCreator, UplinkRemoteSetup};
use lorawan::parser::{self, DataFrameType, DecryptedDataPayload, FrmPayload};

fn handle_multicast_setup_req(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let mut req = McGroupSetupReqCreator::new();
    let mc_addr = McAddr::from_wire_bytes([52, 110, 29, 60]);
    let mc_key = McKey::from([0x44; 16]);
    let mcke_key = McKEKey::from([0x66; 16]);

    req.mc_group_id_header(0x01);
    req.mc_addr(&mc_addr);
    req.mc_key(&DefaultNetworkCrypto::new(mcke_key.inner()), &mc_key);
    req.min_mc_fcount(0x12345678);
    req.max_mc_fcount(0x87654321);
    let setup_req = req.build();

    // Create a downlink frame containing the McGroupSetupReq.
    // The class C setup already delivered a downlink at counter 0, so this one
    // advances the downlink counter.
    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: get_dev_addr(),
        fcnt: 1,
        // Remote multicast setup port
        payload: Payload::Data { f_port: NonZeroU8::new(200).unwrap(), data: setup_req },
        ..Default::default()
    };
    let finished = frame.build_into(rx_buffer, &get_crypto(), Some(&get_crypto())).unwrap();
    finished.len()
}

fn verify_multicast_message(
    uplink: Option<Uplink>,
    expected_port: u8,
    verify_payload: impl FnOnce(&[u8]) -> bool,
) -> usize {
    let mut uplink = uplink.unwrap();
    let bytes = uplink.data_mut();
    let fcnt = match parser::parse(&*bytes) {
        Ok(parser::PhyPayload::Data(data)) => {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_crypto(), fcnt));
            fcnt
        }
        _ => panic!("Expected encrypted data payload"),
    };
    let decrypted = DecryptedDataPayload::decrypt_in_place(
        bytes,
        Some(&get_crypto()),
        Some(&get_crypto()),
        fcnt,
    )
    .unwrap();
    assert_eq!(decrypted.f_port().unwrap(), expected_port);

    if let FrmPayload::Data(ans_data) = decrypted.frm_payload() {
        assert!(verify_payload(ans_data));
    } else {
        panic!("Expected data payload");
    }
    0
}

fn verify_multicast_setup_ans(
    uplink: Option<Uplink>,
    _config: RfConfig,
    _rx_buffer: &mut [u8],
) -> usize {
    verify_multicast_message(uplink, 200, |ans_data| {
        let mut msgs = parse_uplink_multicast_commands(ans_data);
        let msg = msgs.next().unwrap().unwrap();
        if let UplinkRemoteSetup::McGroupSetupAns(ans) = msg {
            assert_eq!(ans.mc_group_id_header(), 0x01);
        } else {
            panic!("Expected McGroupSetupAns");
        }
        assert!(msgs.next().is_none());
        true
    })
}

#[tokio::test]
async fn test_multicast_remote_setup() {
    let (radio, _timer, mut async_device) = util::setup_with_session_class_c().await;

    // Set up McKEKey for the device
    let mcke_key = McKEKey::from([0x66; 16]);
    async_device.mac.multicast.mc_k_e_key = Some(mcke_key);

    // Run the device listening for the setup message
    let task = tokio::spawn(async move {
        let response = async_device.rxc_listen().await;
        (async_device, response)
    });

    // Send the McGroupSetupReq
    radio.handle_rxtx(handle_multicast_setup_req).await;

    // Handle the McGroupSetupAns from the device
    radio.handle_rxtx(verify_multicast_setup_ans).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(ListenResponse::Multicast(MulticastResponse::NewSession { group_id })) => {
            assert_eq!(group_id, 1); // Group ID from the setup request
            // Verify the session was created correctly
            let mc_addr = McAddr::from_wire_bytes([52, 110, 29, 60]);
            let (fetched_group_id, stored_session) =
                device.mac.multicast.matching_session(mc_addr).unwrap();
            assert_eq!(stored_session.multicast_addr(), mc_addr);
            assert_eq!(stored_session.fcnt_down, 0x12345678);
            assert_eq!(stored_session.max_fcnt_down(), 0x87654321);
            assert_eq!(fetched_group_id, 1);
        }
        _ => panic!("Expected NewSession response"),
    }
}

fn handle_mc_group_delete_req<const GROUP_ID: u8>(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let mut req = McGroupDeleteReqCreator::new();
    req.mc_group_id_header(GROUP_ID);
    let setup_req = req.build();

    // Create a downlink frame containing the McGroupDeleteReq
    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: get_dev_addr(),
        fcnt: 2,
        // Remote multicast setup port
        payload: Payload::Data { f_port: NonZeroU8::new(200).unwrap(), data: setup_req },
        ..Default::default()
    };
    let finished = frame.build_into(rx_buffer, &get_crypto(), Some(&get_crypto())).unwrap();
    finished.len()
}

fn verify_mc_group_delete_ans(
    uplink: Option<Uplink>,
    _config: RfConfig,
    _rx_buffer: &mut [u8],
) -> usize {
    verify_multicast_message(uplink, 200, |ans_data| {
        let mut msgs = parse_uplink_multicast_commands(ans_data);
        let msg = msgs.next().unwrap().unwrap();
        if let UplinkRemoteSetup::McGroupDeleteAns(ans) = msg {
            assert_eq!(ans.mc_group_id_header(), 0x01);
            assert!(!ans.mc_group_undefined());
        } else {
            panic!("Expected McGroupDeleteAns");
        }
        assert!(msgs.next().is_none());
        true
    })
}

fn handle_regular_downlink_msg<const FCNT: u32>(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: get_dev_addr(),
        fcnt: FCNT,
        // a random fport that's not the multicast port
        payload: Payload::Data { f_port: NonZeroU8::new(1).unwrap(), data: &[1, 2, 3] },
        ..Default::default()
    };
    let finished = frame.build_into(rx_buffer, &get_crypto(), Some(&get_crypto())).unwrap();
    finished.len()
}

#[tokio::test]
async fn test_multicast_group_delete() {
    let (radio, _timer, mut async_device) = util::setup_with_session_class_c().await;
    let mcke_key = McKEKey::from([0x66; 16]);
    async_device.mac.multicast.mc_k_e_key = Some(mcke_key);

    // Run the device listening for the setup message
    let task = tokio::spawn(async move {
        let response = async_device.rxc_listen().await;
        (async_device, response)
    });

    // Send the McGroupSetupReq
    radio.handle_rxtx(handle_multicast_setup_req).await;

    // Handle the McGroupSetupAns from the device
    radio.handle_rxtx(verify_multicast_setup_ans).await;

    let (mut device, _) = task.await.unwrap();
    // Run the device again so it may listen to the DeleteReq
    let task = tokio::spawn(async move { device.rxc_listen().await });

    // Send the McGroupDeleteReq with correct groupID
    radio.handle_rxtx(handle_mc_group_delete_req::<0x01>).await;
    radio.handle_rxtx(verify_mc_group_delete_ans).await;
    radio.handle_rxtx(handle_regular_downlink_msg::<3>).await;
    let _ = task.await.unwrap();
}

fn verify_mc_group_delete_ans_undefined(
    uplink: Option<Uplink>,
    _config: RfConfig,
    _rx_buffer: &mut [u8],
) -> usize {
    verify_multicast_message(uplink, 200, |ans_data| {
        let mut msgs = parse_uplink_multicast_commands(ans_data);
        let msg = msgs.next().unwrap().unwrap();
        if let UplinkRemoteSetup::McGroupDeleteAns(ans) = msg {
            assert_eq!(ans.mc_group_id_header(), 0x00);
            assert!(ans.mc_group_undefined());
        } else {
            panic!("Expected McGroupDeleteAns");
        }
        assert!(msgs.next().is_none());
        true
    })
}

#[tokio::test]
async fn test_multicast_invalid_group_delete() {
    let (radio, _timer, mut async_device) = util::setup_with_session_class_c().await;
    let mcke_key = McKEKey::from([0x66; 16]);
    async_device.mac.multicast.mc_k_e_key = Some(mcke_key);

    // Run the device listening for the setup message
    let task = tokio::spawn(async move {
        let response = async_device.rxc_listen().await;
        (async_device, response)
    });

    // Send the McGroupSetupReq
    radio.handle_rxtx(handle_multicast_setup_req).await;

    // Handle the McGroupSetupAns from the device
    radio.handle_rxtx(verify_multicast_setup_ans).await;

    let (mut device, _) = task.await.unwrap();
    // Run the device again so it may listen to the DeleteReq
    let task = tokio::spawn(async move { device.rxc_listen().await });

    // Send the McGroupDeleteReq with correct groupID
    radio.handle_rxtx(handle_mc_group_delete_req::<0x03>).await;
    radio.handle_rxtx(verify_mc_group_delete_ans_undefined).await;
    radio.handle_rxtx(handle_regular_downlink_msg::<3>).await;
    let _ = task.await.unwrap();
}
