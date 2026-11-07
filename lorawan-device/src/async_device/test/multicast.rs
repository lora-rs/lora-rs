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
