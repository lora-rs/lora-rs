use super::*;
use crate::async_device::McAddr;
use lorawan::default_crypto::DefaultNetworkCrypto;
use lorawan::keys::{McKEKey, McKey};
use lorawan::multicast::parse_uplink_multicast_commands;
use lorawan::multicast::{McGroupDeleteReqCreator, McGroupSetupReqCreator, UplinkRemoteSetup};
use lorawan::parser::FrmPayload;
use net::{Net, Pkt};

/// Remote multicast setup port
const MC_PORT: u8 = 200;

fn mc_addr() -> McAddr {
    McAddr::from_wire_bytes([52, 110, 29, 60])
}

fn mcke_key() -> McKEKey {
    McKEKey::from([0x66; 16])
}

/// A class C device holding the McKEKey.
async fn setup_multicast() -> Net {
    let mut net = util::setup_with_session_class_c().await;
    net.device().mac.multicast.mc_k_e_key = Some(mcke_key());
    net
}

/// McGroupSetupReq for group 1 with a known McKey and FCnt range.
fn mc_group_setup_req() -> Pkt {
    let mc_key = McKey::from([0x44; 16]);
    let mut req = McGroupSetupReqCreator::new();
    req.mc_group_id_header(0x01);
    req.mc_addr(&mc_addr());
    req.mc_key(&DefaultNetworkCrypto::new(mcke_key().inner()), &mc_key);
    req.min_mc_fcount(0x12345678);
    req.max_mc_fcount(0x87654321);
    Pkt::app(MC_PORT, req.build())
}

fn mc_group_delete_req(group_id: u8) -> Pkt {
    let mut req = McGroupDeleteReqCreator::new();
    req.mc_group_id_header(group_id);
    Pkt::app(MC_PORT, req.build())
}

/// Check the uplink carries exactly one remote multicast setup answer and
/// hand it to `check`.
fn assert_multicast_answer(uplink: &Uplink, check: impl FnOnce(UplinkRemoteSetup<'_>)) {
    let mut uplink = uplink.clone();
    let decrypted = util::decrypt_uplink(&mut uplink);
    assert_eq!(decrypted.f_port(), Some(MC_PORT));
    let FrmPayload::Data(ans_data) = decrypted.frm_payload() else {
        panic!("expected data payload");
    };
    let mut msgs = parse_uplink_multicast_commands(ans_data);
    check(msgs.next().expect("a multicast answer").unwrap());
    assert!(msgs.next().is_none());
}

fn assert_mc_group_setup_ans(uplink: &Uplink) {
    assert_multicast_answer(uplink, |msg| {
        let UplinkRemoteSetup::McGroupSetupAns(ans) = msg else {
            panic!("expected McGroupSetupAns");
        };
        assert_eq!(ans.mc_group_id_header(), 0x01);
    });
}

#[tokio::test]
async fn test_multicast_remote_setup() {
    let mut net = setup_multicast().await;

    let r = net.listen(vec![mc_group_setup_req()]).await;
    let Ok(ListenResponse::Multicast(MulticastResponse::NewSession { group_id })) = r.response()
    else {
        panic!("expected NewSession response, got {:?}", r.response());
    };
    assert_eq!(*group_id, 1); // Group ID from the setup request
    // The device answered with a McGroupSetupAns
    assert_eq!(r.uplinks().len(), 1);
    assert_mc_group_setup_ans(r.uplink(0));

    // Verify the session was created correctly
    let (fetched_group_id, stored_session) =
        net.device().mac.multicast.matching_session(mc_addr()).unwrap();
    assert_eq!(stored_session.multicast_addr(), mc_addr());
    assert_eq!(stored_session.fcnt_down, 0x12345678);
    assert_eq!(stored_session.max_fcnt_down(), 0x87654321);
    assert_eq!(fetched_group_id, 1);
}

#[tokio::test]
async fn test_multicast_group_delete() {
    let mut net = setup_multicast().await;
    let r = net.listen(vec![mc_group_setup_req()]).await;
    assert_mc_group_setup_ans(r.uplink(0));

    // Delete the group, then a regular downlink ends the listen
    let r = net.listen(vec![mc_group_delete_req(0x01), Pkt::app(1, &[1, 2, 3])]).await;
    r.expect_downlink_received();
    assert_eq!(r.uplinks().len(), 1);
    assert_multicast_answer(r.uplink(0), |msg| {
        let UplinkRemoteSetup::McGroupDeleteAns(ans) = msg else {
            panic!("expected McGroupDeleteAns");
        };
        assert_eq!(ans.mc_group_id_header(), 0x01);
        assert!(!ans.mc_group_undefined());
    });
    assert!(net.device().mac.multicast.matching_session(mc_addr()).is_none());
}

#[tokio::test]
async fn test_multicast_invalid_group_delete() {
    let mut net = setup_multicast().await;
    let r = net.listen(vec![mc_group_setup_req()]).await;
    assert_mc_group_setup_ans(r.uplink(0));

    // Delete an unknown group, then a regular downlink ends the listen
    let r = net.listen(vec![mc_group_delete_req(0x03), Pkt::app(1, &[1, 2, 3])]).await;
    r.expect_downlink_received();
    assert_eq!(r.uplinks().len(), 1);
    assert_multicast_answer(r.uplink(0), |msg| {
        let UplinkRemoteSetup::McGroupDeleteAns(ans) = msg else {
            panic!("expected McGroupDeleteAns");
        };
        assert_eq!(ans.mc_group_id_header(), 0x00);
        assert!(ans.mc_group_undefined());
    });
    // Group 1 is untouched
    assert!(net.device().mac.multicast.matching_session(mc_addr()).is_some());
}

/// DutResetReq stands in for an MCU reset, which keeps no multicast state.
#[cfg(feature = "certification")]
#[tokio::test]
async fn test_multicast_state_dropped_on_dut_reset_req() {
    let mut net = setup_multicast().await;
    net.listen(vec![mc_group_setup_req()]).await;
    assert!(net.device().mac.multicast.matching_session(mc_addr()).is_some());

    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::cp_cmd("01"))).await;
    r.expect_rx_complete();

    assert!(net.device().mac.multicast.matching_session(mc_addr()).is_none());
    assert!(net.device().mac.multicast.mc_k_e_key.is_none());
}

/// A remote setup request at the terminal counter reports SessionExpired;
/// the nonzero retransmission counter keeps the session-level expiry from
/// firing first.
#[tokio::test]
async fn test_multicast_setup_expired_reports_session_expired() {
    let mut net = setup_multicast().await;
    let session = net.device().mac.get_session_mut().unwrap();
    session.fcnt_up = 0xFFFF_FFFF;
    session.retransmissions = 1;

    let r = net.listen(vec![mc_group_setup_req()]).await;
    assert!(
        matches!(r.response(), Ok(ListenResponse::SessionExpired)),
        "the setup frame cannot be prepared at the terminal counter, got {:?}",
        r.response()
    );
}

/// A multicast group downlink received in a certification answer's RX1
/// window must be reported to the caller instead of being discarded in
/// favor of `CertificationAnswerSent`.
#[tokio::test]
async fn test_multicast_group_downlink_in_answer_rx1_is_reported() {
    use crate::test_util::{get_crypto, get_dev_addr};
    use core::num::NonZeroU8;
    use lorawan::creator::{DataFrame, Payload};
    use lorawan::default_crypto::DefaultCrypto;
    use lorawan::multicast::{DownlinkRemoteSetup, parse_downlink_multicast_commands};
    use lorawan::parser::DataFrameType;

    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    device.mac.multicast.mc_k_e_key = Some(mcke_key());
    let task = tokio::spawn(async move {
        let _ = device.send(&[1], 1, false).await;
        // The first listen ends on the remote setup request; the second on
        // the certification answer's windows.
        let res1 = device.rxc_listen().await;
        let res2 = device.rxc_listen().await;
        (device, res1, res2)
    });

    // Drive the initial uplink's RX1 and RX2 so the radio enters RXC.
    for _ in 0..2 {
        let armed = timer.get_armed_count().await;
        timer.wait_armed(armed + 1).await;
        timer.fire_most_recent().await;
        radio.handle_timeout().await;
    }

    // Build the remote setup request (FPort 200, session keys, FCntDown 0)
    // and derive the group session keys for the group downlink.
    let mc_key = McKey::from([0x44; 16]);
    let mut req = McGroupSetupReqCreator::new();
    req.mc_group_id_header(0x01);
    req.mc_addr(&mc_addr());
    req.mc_key(&DefaultNetworkCrypto::new(mcke_key().inner()), &mc_key);
    req.min_mc_fcount(0x12345678);
    req.max_mc_fcount(0x87654321);
    let req_bytes = req.build();
    let mut msgs = parse_downlink_multicast_commands(req_bytes);
    let DownlinkRemoteSetup::McGroupSetupReq(req_msg) = msgs.next().unwrap().unwrap() else {
        panic!("expected McGroupSetupReq");
    };
    let (_group_id, group_session) =
        req_msg.derive_session(&DefaultCrypto::new(mcke_key().inner()));

    // Build each frame into its own buffer: they are built before any of
    // them is delivered.
    let crypto = get_crypto();
    let setup = {
        let mut buf = [0u8; 256];
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedDown,
            dev_addr: get_dev_addr(),
            fcnt: 0,
            payload: Payload::Data { f_port: NonZeroU8::new(MC_PORT).unwrap(), data: req_bytes },
            ..Default::default()
        };
        let len = frame.build_into(&mut buf, &crypto, Some(&crypto)).unwrap().len();
        buf[..len].to_vec()
    };
    // Certification command (FCntDown 1); its answer opens its own RX1.
    let cert = {
        let mut buf = [0u8; 256];
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedDown,
            dev_addr: get_dev_addr(),
            fcnt: 1,
            payload: Payload::Data {
                f_port: NonZeroU8::new(224).unwrap(),
                data: &[0x08, 0x01, 0x02, 0x03],
            },
            ..Default::default()
        };
        let len = frame.build_into(&mut buf, &crypto, Some(&crypto)).unwrap().len();
        buf[..len].to_vec()
    };
    // Group downlink: group address, group keys, group FCnt 0, group 1 port.
    let nwk_crypto = DefaultCrypto::new(group_session.mc_net_s_key().inner());
    let app_crypto = DefaultCrypto::new(group_session.mc_app_s_key().inner());
    let group = {
        let mut buf = [0u8; 256];
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedDown,
            dev_addr: 0x3C1D6E34.into(), // mc_addr() as a little-endian u32
            fcnt: 0,
            payload: Payload::Data { f_port: NonZeroU8::new(201).unwrap(), data: &[0xBE, 0xEF] },
            ..Default::default()
        };
        let len = frame.build_into(&mut buf, &nwk_crypto, Some(&app_crypto)).unwrap().len();
        buf[..len].to_vec()
    };

    radio.handle_frame(setup).await;
    radio.handle_frame(cert).await;
    // The answer's RX1 start.
    let armed = timer.get_armed_count().await;
    tokio::time::timeout(tokio::time::Duration::from_secs(5), timer.wait_armed(armed + 1))
        .await
        .expect("the answer must arm its RX1 timer");
    timer.fire_most_recent().await;
    // The group downlink in the answer's RX1.
    radio.handle_frame(group).await;

    let (mut device, res1, res2) = tokio::time::timeout(tokio::time::Duration::from_secs(10), task)
        .await
        .expect("the device must report both downlinks")
        .unwrap();
    let Ok(ListenResponse::Multicast(MulticastResponse::NewSession { group_id })) = res1 else {
        panic!("expected NewSession from the setup request, got {:?}", res1);
    };
    assert_eq!(group_id, 1);
    assert!(
        matches!(
            res2,
            Ok(ListenResponse::Multicast(MulticastResponse::DownlinkReceived {
                group_id: 1,
                fcnt: 0
            }))
        ),
        "expected the group downlink from the answer's RX1, got {:?}",
        res2
    );
    let dl = device.take_downlink().expect("group downlink must be queued");
    assert_eq!(dl.fport, 201);
    assert_eq!(dl.data.as_slice(), &[0xBE, 0xEF]);
}
