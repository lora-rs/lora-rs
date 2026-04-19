use super::*;
use crate::async_device::McAddr;
use lorawan::creator::DataPayloadCreator;
use lorawan::keys::{McKEKey, McKey};
use lorawan::multicast::{
    parse_uplink_multicast_messages, McGroupDeleteReqCreator, McGroupSetupReqCreator,
    UplinkRemoteSetup,
};
use lorawan::parser::{DataHeader, DataPayload, FRMPayload, PhyPayload};

fn handle_multicast_setup_req(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let mut req = McGroupSetupReqCreator::new();
    let mc_addr = McAddr::from([52, 110, 29, 60]);
    let mc_key = McKey::from([0x44; 16]);
    let mcke_key = McKEKey::from([0x66; 16]);

    req.mc_group_id_header(0x01);
    req.mc_addr(&mc_addr);
    req.mc_key(&DefaultFactory, &mc_key, &mcke_key);
    req.min_mc_fcount(0x12345678);
    req.max_mc_fcount(0x87654321);
    let setup_req = req.build();

    // Create a downlink frame containing the McGroupSetupReq
    let mut phy = DataPayloadCreator::new(rx_buffer).unwrap();
    phy.set_f_port(200); // Remote multicast setup port
    phy.set_dev_addr([0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(0);

    let finished =
        phy.build(setup_req, [], &get_key().into(), &get_key().into(), &DefaultFactory).unwrap();
    finished.len()
}

fn verify_multicast_message(
    uplink: Option<Uplink>,
    expected_port: u8,
    verify_payload: impl FnOnce(&[u8]) -> bool,
) -> usize {
    let mut uplink = uplink.unwrap();
    let payload = uplink.get_payload();
    if let PhyPayload::Data(DataPayload::Encrypted(data)) = payload {
        let fcnt = data.fhdr().fcnt() as u32;
        assert!(data.validate_mic(&get_key().into(), fcnt, &DefaultFactory));
        let uplink = data
            .decrypt(Some(&get_key().into()), Some(&get_key().into()), fcnt, &DefaultFactory)
            .unwrap();
        assert_eq!(uplink.f_port().unwrap(), expected_port);

        if let FRMPayload::Data(ans_data) = uplink.frm_payload() {
            assert!(verify_payload(ans_data));
        } else {
            panic!("Expected data payload");
        }
        0
    } else {
        panic!("Expected encrypted data payload");
    }
}

fn verify_multicast_setup_ans(
    uplink: Option<Uplink>,
    _config: RfConfig,
    _rx_buffer: &mut [u8],
) -> usize {
    verify_multicast_message(uplink, 200, |ans_data| {
        let mut msgs = parse_uplink_multicast_messages(ans_data);
        let msg = msgs.next().unwrap();
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
            let mc_addr = McAddr::from([52, 110, 29, 60]);
            let (fetched_group_id, stored_session) = device
                .mac
                .multicast
                .matching_session(McAddr::new(mc_addr.as_ref()).unwrap())
                .unwrap();
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
    let mut phy = DataPayloadCreator::new(rx_buffer).unwrap();
    phy.set_f_port(200); // Remote multicast setup port
    phy.set_dev_addr([0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(1);

    let finished =
        phy.build(setup_req, [], &get_key().into(), &get_key().into(), &DefaultFactory).unwrap();
    finished.len()
}

fn verify_mc_group_delete_ans(
    uplink: Option<Uplink>,
    _config: RfConfig,
    _rx_buffer: &mut [u8],
) -> usize {
    verify_multicast_message(uplink, 200, |ans_data| {
        let mut msgs = parse_uplink_multicast_messages(ans_data);
        let msg = msgs.next().unwrap();
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
    let mut phy = DataPayloadCreator::new(rx_buffer).unwrap();
    phy.set_f_port(1); // a random fport that's not the multicast port
    phy.set_dev_addr([0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(FCNT);

    let finished =
        phy.build(&[1, 2, 3], [], &get_key().into(), &get_key().into(), &DefaultFactory).unwrap();
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
    radio.handle_rxtx(handle_regular_downlink_msg::<2>).await;
    let _ = task.await.unwrap();
}

fn verify_mc_group_delete_ans_undefined(
    uplink: Option<Uplink>,
    _config: RfConfig,
    _rx_buffer: &mut [u8],
) -> usize {
    verify_multicast_message(uplink, 200, |ans_data| {
        let mut msgs = parse_uplink_multicast_messages(ans_data);
        let msg = msgs.next().unwrap();
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
    radio.handle_rxtx(handle_regular_downlink_msg::<2>).await;
    let _ = task.await.unwrap();
}
