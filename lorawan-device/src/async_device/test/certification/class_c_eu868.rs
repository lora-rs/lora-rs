//! LoRaWAN 1.0.4 Class C certification testcases for EU868.
//!
use super::{net, util};
use crate::async_device::ListenResponse;
use net::{ExchangeAction, Net, Pkt};

use lorawan::parser::FrmPayload;

/// Based on TP_C_EU868_ED_MAC_104_BV_000 - Testing for downlinks on different RXC reception slots.
#[cfg(feature = "class-c")]
#[tokio::test]
#[should_panic(expected = "next uplink must advance FCntUp")]
async fn eu868_class_c_echo_payload_req_in_rxc() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;
    // The first uplink leaves the radio listening on RXC.
    net.exchange(&[1], 1, false, ExchangeAction::none()).await.expect_rx_complete();

    let r = net.listen(vec![Pkt::cp_cmd("08010203")]).await;

    assert!(matches!(r.response(), Ok(ListenResponse::CertificationAnswerSent)));
    assert_eq!(r.uplinks().len(), 1, "EchoIncPayloadReq must be answered");
    let mut uplink = r.uplink(0).clone();
    let dl = util::decrypt_uplink(&mut uplink);
    assert_eq!(dl.f_port(), Some(224));
    assert_eq!(dl.frm_payload(), FrmPayload::Data(&[0x08, 0x02, 0x03, 0x04]));
    let answer_fcnt = util::uplink_fcnt(r.uplink(0));

    let r = net.exchange(&[4, 5, 6], 5, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_eq!(util::uplink_fcnt(r.uplink(0)), answer_fcnt + 1, "next uplink must advance FCntUp");
}

/// The answer is transmitted from inside `rxc_listen`. The device must be back
/// in RXC afterwards, or the next downlink is lost.
#[cfg(feature = "class-c")]
#[tokio::test]
async fn eu868_class_c_listens_again_after_answering_in_rxc() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;
    net.exchange(&[1], 1, false, ExchangeAction::none()).await.expect_rx_complete();

    let r = net.listen(vec![Pkt::cp_cmd("08010203")]).await;
    assert!(matches!(r.response(), Ok(ListenResponse::CertificationAnswerSent)));

    let r = net.listen(vec![Pkt::app(1, &[0xAA])]).await;
    assert!(
        matches!(r.response(), Ok(ListenResponse::DownlinkReceived(_))),
        "expected the downlink after the answer, got {:?}",
        r.response()
    );
}

/// Based on TP_C_EU868_ED_MAC_104_BV_000 - Testing for downlinks of an
/// Unconfirmed frame on RX1 and a Confirmed frame on RXC (2-3) window.
///
/// Step 19 sets a 20 s uplink periodicity and Step 20 sends a confirmed
/// TxFramesCtrlReq in RXC. The DUT must acknowledge it immediately, not with
/// the next periodic uplink.
#[cfg(feature = "class-c")]
#[tokio::test]
#[should_panic(expected = "confirmed downlink must be acknowledged")]
async fn eu868_class_c_confirmed_downlink_in_rxc_is_acknowledged() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;

    let action = ExchangeAction::none()
        .in_window(1, Pkt::cp_cmd("0603"))
        .rxc_after(1, Pkt::cp_cmd("0700").confirmed());
    let r = net.exchange(&[1, 2, 3], 3, false, action).await;
    r.expect_rx_complete();
    assert_eq!(net.device().get_override_periodicity(), Some(20));

    assert_eq!(r.uplinks().len(), 2, "confirmed downlink must be acknowledged");
    let mut uplink = r.uplink(1).clone();
    let dl = util::decrypt_uplink(&mut uplink);
    assert!(dl.fhdr().fctrl().ack(), "ACK bit must be set");
    assert!(!dl.is_confirmed(), "the ACK is an unconfirmed uplink");
    assert_eq!(dl.f_port(), None, "an ACK-only uplink has no FPort");
}
