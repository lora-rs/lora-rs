//! LoRaWAN 1.0.4 Class C certification testcases for EU868.
//!
use super::{net, util};
use net::{ExchangeAction, Net, Pkt};

// TODO: XXX: Hangs
/*
/// Based on TP_C_EU868_ED_MAC_104_BV_000 - Testing for downlinks on different RXC reception slots.
#[cfg(feature = "class-c")]
#[tokio::test]
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
    // The answer is a regular uplink: its own RX1 and RX2 windows are run
    // before `rxc_listen` returns.
    assert_eq!(r.windows(), 2, "the answer's RX1 and RX2 must be run");
    let answer_fcnt = util::uplink_fcnt(r.uplink(0));

    let r = net.exchange(&[4, 5, 6], 5, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_eq!(util::uplink_fcnt(r.uplink(0)), answer_fcnt + 1, "next uplink must advance FCntUp");
}
*/

// TODO: XXX: Hangs
/*
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
*/

/// Based on TP_C_EU868_ED_MAC_104_BV_000 - Testing for downlinks of an
/// Unconfirmed frame on RX1 and a Confirmed frame on RXC (2-3) window.
///
/// Step 19 sets a 20 s uplink periodicity and Step 20 sends a confirmed
/// TxFramesCtrlReq in RXC. The DUT must acknowledge it immediately, not with
/// the next periodic uplink.
#[cfg(feature = "class-c")]
// TODO: MAC REWORK
#[should_panic]
#[tokio::test]
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

/// A confirmed certification command that produces no application response
/// (such as `TxPeriodicityChangeReq`) received during `retransmit_wait`
/// must still transmit its immediate ACK rather than leaving it stashed.
#[cfg(feature = "class-c")]
// TODO: MAC REWORK
#[should_panic]
#[tokio::test]
async fn eu868_class_c_confirmed_tx_periodicity_in_retransmit_wait_is_acked() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;
    net.exchange(&[1], 1, false, ExchangeAction::none()).await.expect_rx_complete();

    let action = ExchangeAction::none().rxc_after(2, Pkt::cp_cmd("0603").confirmed());
    let r = net.exchange(&[1, 2, 3], 3, true, action).await;
    assert_eq!(net.device().get_override_periodicity(), Some(20));

    assert_eq!(r.uplinks().len(), 2, "confirmed TxPeriodicityChangeReq must be acknowledged");
    let mut ack = r.uplink(1).clone();
    let dl = util::decrypt_uplink(&mut ack);
    assert!(dl.fhdr().fctrl().ack(), "ACK bit must be set");
    assert!(!dl.is_confirmed(), "the ACK is an unconfirmed uplink");
    assert_eq!(dl.f_port(), None, "an ACK-only uplink has no FPort");
}

// TODO: XXX: Hangs
/*
/// A downlink received during a certification answer's RX1 window inside
/// `rxc_listen` must be reported to the caller instead of being discarded in
/// favor of `CertificationAnswerSent`.
#[cfg(feature = "class-c")]
#[tokio::test]
async fn eu868_class_c_rxc_listen_reports_downlink_in_answer_rx1() {
    use crate::test_util::{get_crypto, get_dev_addr};
    use core::num::NonZeroU8;
    use lorawan::creator::{DataFrame, Payload};
    use lorawan::parser::DataFrameType;

    fn build_dl(fcnt: u32, fport: u8, data: &[u8]) -> std::vec::Vec<u8> {
        let mut buf = [0u8; 256];
        let crypto = get_crypto();
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedDown,
            dev_addr: get_dev_addr(),
            adr: false,
            adr_ack_req: false,
            ack: false,
            f_pending: false,
            fcnt,
            f_opts: &[],
            payload: Payload::Data { f_port: NonZeroU8::new(fport).unwrap(), data },
        };
        frame.build_into(&mut buf, &crypto, Some(&crypto)).unwrap().to_vec()
    }

    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let task = tokio::spawn(async move {
        let _ = device.send(&[1], 1, false).await;
        let res = device.rxc_listen().await;
        (device, res)
    });

    // Drive the initial uplink's RX1 and RX2 so the radio enters RXC.
    for _ in 0..2 {
        let armed = timer.get_armed_count().await;
        timer.wait_armed(armed + 1).await;
        timer.fire_most_recent().await;
        radio.handle_timeout().await;
    }

    // Deliver EchoIncPayloadReq in RXC; the answer opens its own RX1.
    radio.handle_frame(build_dl(0, 224, &[0x08, 1, 2, 3])).await;
    let armed = timer.get_armed_count().await;
    timer.wait_armed(armed + 1).await;
    timer.fire_most_recent().await;
    // Deliver an application downlink in the answer's RX1.
    radio.handle_frame(build_dl(1, 5, &[0xDE, 0xAD])).await;

    let (mut device, res) = task.await.unwrap();
    assert!(
        matches!(res, Ok(ListenResponse::DownlinkReceived(1))),
        "expected DownlinkReceived(1) from the answer's RX1, got {:?}",
        res
    );
    let dl = device.take_downlink().expect("downlink must be queued");
    assert_eq!(dl.fport, 5);
    assert_eq!(dl.data.as_slice(), &[0xDE, 0xAD]);
}
*/
