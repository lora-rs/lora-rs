//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.4
//!
//! EU868 LinkADRReq tests

use super::{decrypt_uplink, net, util};
use crate::test_util::Uplink;
use lorawan::parser::FrmPayload;

use net::{ExchangeAction, Net, Pkt};

/// DUT helper: parse, MIC-check and decrypt an uplink, then verify its
/// header fields and (decrypted) FRMPayload.
fn assert_uplink(
    uplink: &Uplink,
    fcnt: u32,
    confirmed: bool,
    ack: bool,
    fopts: &[u8],
    fport: Option<u8>,
    payload: &[u8],
) {
    let mut uplink = uplink.clone();
    let dl = decrypt_uplink(&mut uplink);
    assert_eq!(dl.fhdr().fcnt() as u32, fcnt, "FCntUp");
    assert_eq!(dl.is_confirmed(), confirmed, "frame type");
    assert_eq!(dl.fhdr().fctrl().ack(), ack, "ACK bit");
    assert_eq!(dl.fhdr().f_opts(), fopts, "FOpts");
    assert_eq!(dl.f_port(), fport, "FPort");
    match (dl.frm_payload(), payload) {
        (FrmPayload::Data(data), _) => assert_eq!(data, payload, "FRMPayload"),
        (FrmPayload::MacCommands(data), _) => assert_eq!(data, payload, "FRMPayload"),
        (FrmPayload::None, _) => assert!(payload.is_empty(), "expected no FRMPayload"),
    }
}

/// Check an uplink is a plain application data frame on port 3.
fn assert_data_uplink(uplink: &Uplink, fcnt: u32, confirmed: bool, ack: bool, fopts: &[u8]) {
    assert_uplink(uplink, fcnt, confirmed, ack, fopts, Some(3), &[1, 2, 3]);
}

#[tokio::test]
/// EU868 LinkADRReq Redundancy test
///
/// This test validates the DUT's correct implementation of the NbTrans
/// setting within the LinkADRReq command.
///
/// Like a regular uplink, a certification answer (FPort 224) opens its own
/// RX windows and is retransmitted per NbTrans. One deviation from the
/// paper procedure: the paper test sends some TCL downlinks in the answer's
/// own RX windows; here those windows are kept silent and the downlinks are
/// delivered in the next regular uplink's window instead.
async fn eu868_linkadrreq_redundancy() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // LinkADRReq used throughout: CID 0x03, DRTP 0x50 (DataRate = DR5 =
    // Max125kHzDR, TXPower = 0 = maximum), ChMask [0x07, 0x00] (ChMaskCntl =
    // 0 with only the default channels 0..2 enabled), Redundancy byte whose
    // lower nibble is NbTrans (ChMaskCntl lives in the upper nibble in this
    // implementation).
    const LINKADR_REQ: &str = "03500700";
    const LINKADR_ANS: &[u8] = &[0x03, 0x07]; // all three ACK bits set

    /// LinkADRReq with the given NbTrans (0 = default of 1 transmission).
    fn linkadr_req(nb_trans: u8) -> Pkt {
        Pkt::mac(&format!("{LINKADR_REQ}{nb_trans:02x}"))
    }

    /// CP-CMD RxAppCntReq (FPort 224, payload 0x09).
    fn rxappcnt_req() -> Pkt {
        Pkt::cp_cmd("09")
    }

    // Step 1: DUT sends an unconfirmed frame (FCntUp = n = 0); the TCL
    // responds on RX1 with CP-CMD RxAppCntReq. The answer (FCntUp = n + 1)
    // opens its own RX windows, which the exchange keeps silent.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(rxappcnt_req())).await;
    r.expect_rx_complete();

    // Step 2: DUT answers with an RxAppCntAns uplink (FCntUp = n + 1)
    // reporting RxAppCnt = x. The device sends the answer immediately, so
    // this uplink was transmitted inside the step-1 uplink's window.
    let x: u16 = 1;
    assert_uplink(r.uplink(1), 1, false, false, &[], Some(224), &[0x09, 0x01, 0x00]);

    // Step 3: DUT sends an unconfirmed frame (FCntUp = n + 2); the TCL
    // responds with MAC-CMD LinkADRReq, NbTrans = 2.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(linkadr_req(2))).await;
    r.expect_downlink_received();
    // The next uplink must carry LinkADRAns (all settings valid)
    let session = net.device().mac.get_session().unwrap();
    assert_eq!(session.uplink.mac_commands(), LINKADR_ANS);

    // Steps 4-5: the uplink (FCntUp = n + 3) carries the LinkADRAns and, as
    // it receives no downlink, is retransmitted once (NbTrans = 2) with the
    // same FCntUp.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_data_uplink(r.uplink(0), 3, false, false, LINKADR_ANS);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(r.uplink(0).data(), r.uplink(1).data());

    // Steps 6-7: next uplink (FCntUp = n + 4) is also sent twice
    // (NbTrans = 2 is still in effect).
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_data_uplink(r.uplink(0), 4, false, false, &[]);
    assert_eq!(r.uplink(0).data(), r.uplink(1).data());

    // Step 8: DUT uplink (FCntUp = n + 5); the TCL responds with
    // RxAppCntReq on the RX1 window.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(rxappcnt_req())).await;
    r.expect_rx_complete();

    // Step 9: the RxAppCntAns uplink (FCntUp = n + 6) reports
    // RxAppCnt = x + 1. The answer is a regular uplink: it opens its own
    // RX windows (kept silent) and, as it receives no downlink, is
    // retransmitted once (NbTrans = 2) with the same FCntUp.
    assert_uplink(r.uplink(1), 6, false, false, &[], Some(224), &[0x09, (x + 1) as u8, 0x00]);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(r.uplink(1).data(), r.uplink(2).data());

    // Step 11: DUT uplink (FCntUp = n + 7); the TCL responds with
    // RxAppCntReq on the RX2 window.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx2(rxappcnt_req())).await;
    r.expect_rx_complete();

    // Step 12: the RxAppCntAns uplink (FCntUp = n + 8) reports
    // RxAppCnt = x + 2. The answer is a regular uplink: it opens its own
    // RX windows (kept silent) and, as it receives no downlink, is
    // retransmitted once (NbTrans = 2) with the same FCntUp.
    assert_uplink(r.uplink(1), 8, false, false, &[], Some(224), &[0x09, (x + 2) as u8, 0x00]);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(r.uplink(1).data(), r.uplink(2).data());

    // Step 12 (cont'd) / 13: the TCL sends a LinkADRReq with NbTrans = 1.
    // The answer's RX windows above are kept silent, so the TCL delivers it
    // in the next regular uplink's window (FCntUp = n + 9) instead.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(linkadr_req(1))).await;
    r.expect_downlink_received();

    // Step 14: the next uplink (FCntUp = n + 10) carries the LinkADRAns
    // ("uplink sent once", NbTrans = 1) and the TCL responds with
    // CP-CMD TxFramesCtrlReq (frame type = Confirmed).
    let r =
        net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("0702").ack())).await;
    r.expect_downlink_received();
    assert_data_uplink(r.uplink(0), 10, false, false, LINKADR_ANS);
    // The session is now configured to send only confirmed frames
    let session = net.device().mac.get_session().unwrap();
    assert_eq!(session.override_confirmed, Some(true));

    // Step 15: the DUT sends a confirmed frame (FCntUp = n + 11) although
    // the application asked for unconfirmed; the TCL acknowledges it and
    // responds (confirmed downlink) with a LinkADRReq, NbTrans = 3.
    let r = net
        .exchange(
            &[1, 2, 3],
            3,
            false,
            ExchangeAction::rx1(Pkt::mac(&format!("{LINKADR_REQ}03")).confirmed().ack()),
        )
        .await;
    r.expect_downlink_received();
    assert_data_uplink(r.uplink(0), 11, true, false, &[]);

    // Steps 16-18: the next confirmed uplink (FCntUp = n + 12) carries the
    // LinkADRAns and, as it is never acknowledged, is sent three times
    // (NbTrans = 3) with the same FCntUp.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
    r.expect_no_ack();
    assert_eq!(r.uplinks().len(), 3);
    for tx in r.uplinks() {
        assert_data_uplink(tx, 12, true, true, LINKADR_ANS);
    }
    let bytes: Vec<Vec<u8>> = r.uplinks().iter().map(|tx| tx.data().to_vec()).collect();
    assert_eq!(bytes[0], bytes[1]);
    assert_eq!(bytes[1], bytes[2]);

    // Step 19: the next uplink (FCntUp = n + 13) is still confirmed (the
    // override is in effect until told otherwise); the TCL acknowledges the
    // previous frame and responds with CP-CMD TxFramesCtrlReq (frame type =
    // Unconfirmed) to revert.
    let r =
        net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("0701").ack())).await;
    r.expect_downlink_received();
    assert_data_uplink(r.uplink(0), 13, true, false, &[]);
    let session = net.device().mac.get_session().unwrap();
    assert_eq!(session.override_confirmed, Some(false));

    // Step 20: the next uplink (FCntUp = n + 14) is unconfirmed again; the
    // TCL responds with RxAppCntReq.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(rxappcnt_req())).await;

    // Steps 21-23: the RxAppCntAns uplink (FCntUp = n + 15) reports the
    // number of applicative downlinks the DUT has received: the three
    // RxAppCntReqs, the two TxFramesCtrlReqs, plus the step-15 downlink,
    // which carries no application data but acknowledges the previous
    // confirmed uplink (an empty downlink frame with the ACK bit set is an
    // applicative downlink per the certification spec), i.e. x + 6.
    // No Ack bit: the step-19 TxFramesCtrlReq downlink is unconfirmed, so
    // there is no confirmed downlink left to acknowledge. The answer is a
    // regular uplink: it opens its own RX windows (kept silent) and, as it
    // receives no downlink, is sent three times (NbTrans = 3) with the same
    // FCntUp.
    r.expect_rx_complete();
    assert_eq!(r.uplinks().len(), 4);
    for tx in r.uplinks()[1..].iter() {
        assert_uplink(tx, 15, false, false, &[], Some(224), &[0x09, (x + 6) as u8, 0x00]);
    }

    // Step 23 (cont'd) / 24: the TCL sends a LinkADRReq with NbTrans = 0,
    // which means the default of 1 transmission. The answer's RX windows
    // above are kept silent, so the TCL delivers it in the next regular
    // uplink's window (FCntUp = n + 16) instead.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(linkadr_req(0))).await;
    r.expect_downlink_received();

    // Step 24: the uplink carrying the LinkADRAns (FCntUp = n + 17) is sent
    // only once.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_data_uplink(r.uplink(0), 17, false, false, LINKADR_ANS);

    // Step 25: DUT uplink (FCntUp = n + 18); the TCL responds with a
    // LinkADRReq with NbTrans = 1.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(linkadr_req(1))).await;
    r.expect_downlink_received();

    // Step 26: the uplink (FCntUp = n + 19) carries the LinkADRAns; the DUT
    // has reverted to the default settings.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_data_uplink(r.uplink(0), 19, false, false, LINKADR_ANS);

    // Final state: all 20 FCntUps (n .. n + 19) were consumed, the DUT
    // received 7 applicative downlinks (six FPort > 0, plus the step-15
    // downlink without application data carrying the ACK bit), the
    // confirmed override is off and the LinkADR settings hold DR5 / maximum
    // power / NbTrans = 1.
    let session = net.device().mac.get_session().unwrap();
    assert_eq!(session.fcnt_up, 20);
    assert_eq!(session.rx_app_cnt, 7);
    assert_eq!(session.override_confirmed, Some(false));
    let config = &net.device().mac.configuration;
    assert_eq!(config.data_rate, crate::region::DR::_5);
    assert_eq!(config.tx_power, Some(16)); // EU868 maximum EIRP
    assert_eq!(config.nb_trans, 1);
}
