use super::*;
use crate::async_device::McAddr;
use lorawan::default_crypto::DefaultNetworkCrypto;
use lorawan::keys::{McKEKey, McKey};
use lorawan::multicast::parse_uplink_multicast_commands;
use lorawan::multicast::{McGroupDeleteReqCreator, McGroupSetupReqCreator, UplinkRemoteSetup};
use lorawan::parser::FrmPayload;
use net::{ExchangeAction, Net, Pkt};

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

    // The setup downlink stages the answer; it is sent on the next uplink.
    let r = net.listen(vec![mc_group_setup_req()]).await;
    let Ok(ListenResponse::Multicast(MulticastResponse::NewSession { group_id })) = r.response()
    else {
        panic!("expected NewSession response, got {:?}", r.response());
    };
    assert_eq!(*group_id, 1); // Group ID from the setup request
    assert_eq!(r.uplinks().len(), 0);

    // The next regular uplink carries the McGroupSetupAns instead of the app data.
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
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

/// A PackageVersionReq in the app uplink's RX2 window is answered right
/// away: the exchange transmits the app data, then the answer.
#[tokio::test]
async fn test_multicast_package_version_answered_immediately() {
    let mut net = setup_multicast().await;
    let r =
        net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx2(Pkt::app(MC_PORT, &[0x00]))).await;
    r.expect_rx_complete();
    assert_eq!(r.uplinks().len(), 2, "the answer is sent right after the app uplink");
    assert_multicast_answer(r.uplink(1), |msg| {
        let UplinkRemoteSetup::PackageVersionAns(ans) = msg else {
            panic!("expected PackageVersionAns");
        };
        assert_eq!(ans.package_identifier(), 2);
        assert_eq!(ans.package_version(), 2);
    });
}

#[tokio::test]
async fn test_multicast_group_delete() {
    let mut net = setup_multicast().await;
    // Stage the setup answer; it is sent on the next uplink.
    let r = net.listen(vec![mc_group_setup_req()]).await;
    assert_eq!(r.uplinks().len(), 0);
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_mc_group_setup_ans(r.uplink(0));

    // Delete the group: the delete answer is staged and sent on the next uplink.
    let r = net.listen(vec![mc_group_delete_req(0x01), Pkt::app(1, &[1, 2, 3])]).await;
    r.expect_downlink_received();
    assert_eq!(r.uplinks().len(), 0);
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
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
    // Stage the setup answer; it is sent on the next uplink.
    let r = net.listen(vec![mc_group_setup_req()]).await;
    assert_eq!(r.uplinks().len(), 0);
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_mc_group_setup_ans(r.uplink(0));

    // Delete an unknown group: the delete answer is staged and sent on the next uplink.
    let r = net.listen(vec![mc_group_delete_req(0x03), Pkt::app(1, &[1, 2, 3])]).await;
    r.expect_downlink_received();
    assert_eq!(r.uplinks().len(), 0);
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
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

/// A remote setup request at the terminal counter is staged, but the answer
/// cannot be sent on the next uplink: the exchange reports SessionExpired.
#[tokio::test]
async fn test_multicast_setup_expired_reports_session_expired() {
    let mut net = setup_multicast().await;
    let session = net.device().mac.get_session_mut().unwrap();
    session.fcnt_up = 0xFFFF_FFFF;
    session.retransmissions = 1;

    // The setup is staged; it is sent on the next uplink.
    let r = net.listen(vec![mc_group_setup_req()]).await;
    assert!(
        matches!(r.response(), Ok(ListenResponse::Multicast(MulticastResponse::NewSession { .. }))),
        "the setup is staged, got {:?}",
        r.response()
    );
    // The terminal counter prevents the answer from being prepared on the next uplink.
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
    assert!(
        matches!(r.response(), Ok(SendResponse::SessionExpired)),
        "the setup frame cannot be prepared at the terminal counter, got {:?}",
        r.response()
    );
}
