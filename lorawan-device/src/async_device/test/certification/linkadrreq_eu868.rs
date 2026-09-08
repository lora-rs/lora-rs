//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.4
//!
//! EU868 LinkADRReq tests

use super::{decrypt_uplink, util};
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{Uplink, get_crypto, get_dev_addr};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::parser::{self, DataFrameType, FrmPayload, PhyPayload};

/// Build a downlink frame. `fport = 0` puts the payload in FRMPayload as MAC
/// commands; any other value is an FPort data payload. A confirmed
/// `frame_type` acknowledges the previous confirmed uplink.
fn build_downlink(
    buf: &mut [u8],
    frame_type: DataFrameType,
    ack: bool,
    fport: u8,
    payload_hex: &str,
    fcnt: u16,
) -> usize {
    let payload = hex::decode(payload_hex).unwrap();
    let frame = DataFrame {
        frame_type,
        dev_addr: get_dev_addr(),
        ack,
        fcnt: fcnt.into(),
        payload: match fport {
            0 => Payload::MacCommands(&payload),
            p => Payload::Data { f_port: NonZeroU8::new(p).unwrap(), data: &payload },
        },
        ..Default::default()
    };
    let finished = frame.build_into(buf, &get_crypto(), Some(&get_crypto())).unwrap();
    finished.len()
}

/// TCL helper: sanity-check the FCntUp of the uplink the downlink answers.
fn expect_uplink_fcnt(uplink: Option<Uplink>, fcnt: u32) {
    let Some(mut uplink) = uplink else {
        panic!("expected an uplink");
    };
    match parser::parse(uplink.data_mut()) {
        Ok(PhyPayload::Data(data)) => {
            assert_eq!(data.fhdr().fcnt() as u32, fcnt, "answered the wrong uplink");
        }
        _ => panic!("expected a data uplink"),
    }
}

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
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());

    // LinkADRReq used throughout: CID 0x03, DRTP 0x50 (DataRate = DR5 =
    // Max125kHzDR, TXPower = 0 = maximum), ChMask [0x07, 0x00] (ChMaskCntl =
    // 0 with only the default channels 0..2 enabled), Redundancy byte whose
    // lower nibble is NbTrans (ChMaskCntl lives in the upper nibble in this
    // implementation).
    const LINKADR_REQ: &str = "03500700";
    const LINKADR_ANS: &[u8] = &[0x03, 0x07]; // all three ACK bits set

    /// LinkADRReq with the given NbTrans (0 = default of 1 transmission).
    fn linkadr_req(buf: &mut [u8], nb_trans: u8, fcnt: u16) -> usize {
        let payload = format!("{LINKADR_REQ}{nb_trans:02x}");
        build_downlink(buf, DataFrameType::UnconfirmedDown, false, 0, &payload, fcnt)
    }

    /// CP-CMD RxAppCntReq (FPort 224, payload 0x09).
    fn rxappcnt_req(buf: &mut [u8], fcnt: u16) -> usize {
        build_downlink(buf, DataFrameType::UnconfirmedDown, false, 224, "09", fcnt)
    }

    // Step 1: DUT sends an unconfirmed frame (FCntUp = n = 0); the TCL
    // responds on RX1 with CP-CMD RxAppCntReq.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_rxappcnt_req_1(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 0);
        rxappcnt_req(buf, 0)
    }
    radio.handle_rxtx(tcl_rxappcnt_req_1).await;

    // The answer (FCntUp = n + 1) opens its own RX windows.
    // (fire_when_armed: the device arms this timer as a result of the
    // downlink above, so the test must not race the device task.)
    timer.fire_when_armed(2).await; // answer RX1 start
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete after RxAppCntReq, got {response:?}"),
    }

    // Step 2: DUT answers with an RxAppCntAns uplink (FCntUp = n + 1)
    // reporting RxAppCnt = x. The device sends the answer immediately, so
    // this uplink was transmitted inside the window of the step-1 uplink.
    let uplink = radio.get_last_uplink().await;
    assert_uplink(&uplink, 1, false, false, &[], Some(224), &[0x09, 0x01, 0x00]);

    let x: u16 = 1;

    // Step 3: DUT sends an unconfirmed frame (FCntUp = n + 2); the TCL
    // responds with MAC-CMD LinkADRReq, NbTrans = 2.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_linkadr_req_nbtrans2(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 2);
        linkadr_req(buf, 2, 1)
    }
    radio.handle_rxtx(tcl_linkadr_req_nbtrans2).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }
    // The next uplink must carry LinkADRAns (all settings valid)
    let session = device.mac.get_session().unwrap();
    assert_eq!(session.uplink.mac_commands(), LINKADR_ANS);

    // Steps 4-5: the uplink (FCntUp = n + 3) carries the LinkADRAns and, as
    // it receives no downlink, is retransmitted once (NbTrans = 2) with the
    // same FCntUp.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    // Transmission 1
    timer.fire_most_recent().await; // RX1 start
    let mut tx1 = radio.get_last_uplink().await;
    radio.handle_timeout().await; // RX1 end
    timer.fire_most_recent().await; // RX2 start
    radio.handle_timeout().await; // RX2 end -> retransmit
    // Transmission 2
    timer.fire_most_recent().await; // RX1 start
    let mut tx2 = radio.get_last_uplink().await;
    radio.handle_timeout().await; // RX1 end
    timer.fire_most_recent().await; // RX2 start
    radio.handle_timeout().await; // RX2 end -> FCntUp consumed

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }
    assert_data_uplink(&tx1, 3, false, false, LINKADR_ANS);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(tx1.data_mut(), tx2.data_mut());

    // Steps 6-7: next uplink (FCntUp = n + 4) is also sent twice
    // (NbTrans = 2 is still in effect).
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    let mut tx1 = radio.get_last_uplink().await;
    radio.handle_timeout().await;
    timer.fire_most_recent().await; // RX2 start
    radio.handle_timeout().await; // -> retransmit
    timer.fire_most_recent().await; // RX1 start
    let mut tx2 = radio.get_last_uplink().await;
    radio.handle_timeout().await;
    timer.fire_most_recent().await; // RX2 start
    radio.handle_timeout().await; // -> FCntUp consumed

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }
    assert_data_uplink(&tx1, 4, false, false, &[]);
    assert_eq!(tx1.data_mut(), tx2.data_mut());

    // Step 8: DUT uplink (FCntUp = n + 5); the TCL responds with
    // RxAppCntReq on the RX1 window.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_rxappcnt_req_rx1(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 5);
        rxappcnt_req(buf, 2)
    }
    radio.handle_rxtx(tcl_rxappcnt_req_rx1).await;

    // Step 9: the RxAppCntAns uplink (FCntUp = n + 6) reports
    // RxAppCnt = x + 1. The answer is a regular uplink: it opens its own
    // RX windows and, as it receives no downlink, is retransmitted once
    // (NbTrans = 2) with the same FCntUp.
    // Transmission 1
    timer.fire_when_armed(14).await; // answer RX1 start
    let mut tx1 = radio.get_last_uplink().await;
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end -> retransmit
    // Transmission 2
    timer.fire_most_recent().await; // answer RX1 start
    let mut tx2 = radio.get_last_uplink().await;
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end -> FCntUp consumed

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }
    assert_uplink(&tx1, 6, false, false, &[], Some(224), &[0x09, (x + 1) as u8, 0x00]);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(tx1.data_mut(), tx2.data_mut());

    // Step 11: DUT uplink (FCntUp = n + 7); the TCL responds with
    // RxAppCntReq on the RX2 window.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    radio.handle_timeout().await; // RX1 end
    timer.fire_most_recent().await; // RX2 start
    fn tcl_rxappcnt_req_rx2(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 7);
        rxappcnt_req(buf, 3)
    }
    radio.handle_rxtx(tcl_rxappcnt_req_rx2).await;

    // Step 12: the RxAppCntAns uplink (FCntUp = n + 8) reports
    // RxAppCnt = x + 2. The answer is a regular uplink: it opens its own
    // RX windows and, as it receives no downlink, is retransmitted once
    // (NbTrans = 2) with the same FCntUp.
    // Transmission 1
    timer.fire_when_armed(20).await; // answer RX1 start
    let mut tx1 = radio.get_last_uplink().await;
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end -> retransmit
    // Transmission 2
    timer.fire_most_recent().await; // answer RX1 start
    let mut tx2 = radio.get_last_uplink().await;
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end -> FCntUp consumed

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }
    assert_uplink(&tx1, 8, false, false, &[], Some(224), &[0x09, (x + 2) as u8, 0x00]);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(tx1.data_mut(), tx2.data_mut());

    // Step 12 (cont'd) / 13: the TCL sends a LinkADRReq with NbTrans = 1.
    // The answer's RX windows above are kept silent, so the TCL delivers it
    // in the next regular uplink's window (FCntUp = n + 9) instead.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_linkadr_req_nbtrans1(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 9);
        linkadr_req(buf, 1, 4)
    }
    radio.handle_rxtx(tcl_linkadr_req_nbtrans1).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(4)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Step 14: the next uplink (FCntUp = n + 10) carries the LinkADRAns
    // ("uplink sent once", NbTrans = 1) and the TCL responds with
    // CP-CMD TxFramesCtrlReq (frame type = Confirmed).
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_txframectrl_confirmed(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 10);
        build_downlink(buf, DataFrameType::UnconfirmedDown, true, 224, "0702", 5)
    }
    radio.handle_rxtx(tcl_txframectrl_confirmed).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(5)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }
    let uplink = radio.get_last_uplink().await;
    assert_data_uplink(&uplink, 10, false, false, LINKADR_ANS);
    // The session is now configured to send only confirmed frames
    let session = device.mac.get_session().unwrap();
    assert_eq!(session.override_confirmed, Some(true));

    // Step 15: the DUT sends a confirmed frame (FCntUp = n + 11) although
    // the application asked for unconfirmed; the TCL acknowledges it and
    // responds (confirmed downlink) with a LinkADRReq, NbTrans = 3.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_linkadr_req_nbtrans3(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 11);
        let payload = format!("{LINKADR_REQ}03");
        build_downlink(buf, DataFrameType::ConfirmedDown, true, 0, &payload, 6)
    }
    radio.handle_rxtx(tcl_linkadr_req_nbtrans3).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(6)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }
    let uplink = radio.get_last_uplink().await;
    assert_data_uplink(&uplink, 11, true, false, &[]);

    // Steps 16-18: the next confirmed uplink (FCntUp = n + 12) carries the
    // LinkADRAns and, as it is never acknowledged, is sent three times
    // (NbTrans = 3) with the same FCntUp.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    let mut transmissions = Vec::new();
    for _ in 0..3 {
        timer.fire_most_recent().await; // RX1 start
        transmissions.push(radio.get_last_uplink().await);
        radio.handle_timeout().await; // RX1 end
        timer.fire_most_recent().await; // RX2 start
        radio.handle_timeout().await; // RX2 end
    }

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::NoAck) => {}
        _ => panic!("expected NoAck after 3 unacknowledged confirmed uplinks, got {response:?}"),
    }
    for tx in &transmissions {
        assert_data_uplink(tx, 12, true, true, LINKADR_ANS);
    }
    let bytes: Vec<Vec<u8>> = transmissions.iter_mut().map(|tx| tx.data_mut().to_vec()).collect();
    assert_eq!(bytes[0], bytes[1]);
    assert_eq!(bytes[1], bytes[2]);

    // Step 19: the next uplink (FCntUp = n + 13) is still confirmed (the
    // override is in effect until told otherwise); the TCL acknowledges the
    // previous frame and responds with CP-CMD TxFramesCtrlReq (frame type =
    // Unconfirmed) to revert.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_txframectrl_unconfirmed(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 13);
        build_downlink(buf, DataFrameType::UnconfirmedDown, true, 224, "0701", 7)
    }
    radio.handle_rxtx(tcl_txframectrl_unconfirmed).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(7)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }
    let uplink = radio.get_last_uplink().await;
    assert_data_uplink(&uplink, 13, true, false, &[]);
    let session = device.mac.get_session().unwrap();
    assert_eq!(session.override_confirmed, Some(false));

    // Step 20: the next uplink (FCntUp = n + 14) is unconfirmed again; the
    // TCL responds with RxAppCntReq.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_rxappcnt_req_4(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 14);
        rxappcnt_req(buf, 8)
    }
    radio.handle_rxtx(tcl_rxappcnt_req_4).await;

    // Steps 21-23: the RxAppCntAns uplink (FCntUp = n + 15) reports the
    // number of applicative downlinks the DUT has received: the three
    // RxAppCntReqs, the two TxFramesCtrlReqs, plus the step-15 downlink,
    // which carries no application data but acknowledges the previous
    // confirmed uplink (an empty downlink frame with the ACK bit set is an
    // applicative downlink per the certification spec), i.e. x + 6.
    // No Ack bit: the step-19 TxFramesCtrlReq downlink is unconfirmed, so
    // there is no confirmed downlink left to acknowledge. The answer is a
    // regular uplink: it opens its own RX windows and, as it receives no
    // downlink, is sent three times (NbTrans = 3) with the same FCntUp.
    // (fire_when_armed(35): the armed count is monotonic, so this only
    // blocks on the first iteration, for the first answer RX1 start.)
    let mut transmissions = Vec::new();
    for _ in 0..3 {
        timer.fire_when_armed(35).await; // answer RX1 start
        transmissions.push(radio.get_last_uplink().await);
        radio.handle_timeout().await; // answer RX1 end
        timer.fire_most_recent().await; // answer RX2 start
        radio.handle_timeout().await; // answer RX2 end
    }

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }
    for tx in &transmissions {
        assert_uplink(tx, 15, false, false, &[], Some(224), &[0x09, (x + 6) as u8, 0x00]);
    }

    // Step 23 (cont'd) / 24: the TCL sends a LinkADRReq with NbTrans = 0,
    // which means the default of 1 transmission. The answer's RX windows
    // above are kept silent, so the TCL delivers it in the next regular
    // uplink's window (FCntUp = n + 16) instead.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_linkadr_req_nbtrans0(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 16);
        linkadr_req(buf, 0, 9)
    }
    radio.handle_rxtx(tcl_linkadr_req_nbtrans0).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(9)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Step 24: the uplink carrying the LinkADRAns (FCntUp = n + 17) is sent
    // only once.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    let tx = radio.get_last_uplink().await;
    radio.handle_timeout().await; // RX1 end
    timer.fire_most_recent().await; // RX2 start
    radio.handle_timeout().await; // RX2 end

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }
    assert_data_uplink(&tx, 17, false, false, LINKADR_ANS);

    // Step 25: DUT uplink (FCntUp = n + 18); the TCL responds with a
    // LinkADRReq with NbTrans = 1.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn tcl_linkadr_req_nbtrans1_final(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 18);
        linkadr_req(buf, 1, 10)
    }
    radio.handle_rxtx(tcl_linkadr_req_nbtrans1_final).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(10)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Step 26: the uplink (FCntUp = n + 19) carries the LinkADRAns; the DUT
    // has reverted to the default settings.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    let tx = radio.get_last_uplink().await;
    radio.handle_timeout().await; // RX1 end
    timer.fire_most_recent().await; // RX2 start
    radio.handle_timeout().await; // RX2 end

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }
    assert_data_uplink(&tx, 19, false, false, LINKADR_ANS);

    // Final state: all 20 FCntUps (n .. n + 19) were consumed, the DUT
    // received 7 applicative downlinks (six FPort > 0, plus the step-15
    // downlink without application data carrying the ACK bit), the
    // confirmed override is off and the LinkADR settings hold DR5 / maximum
    // power / NbTrans = 1.
    let session = device.mac.get_session().unwrap();
    assert_eq!(session.fcnt_up, 20);
    assert_eq!(session.rx_app_cnt, 7);
    assert_eq!(session.override_confirmed, Some(false));
    let config = &device.mac.configuration;
    assert_eq!(config.data_rate, crate::region::DR::_5);
    assert_eq!(config.tx_power, Some(16)); // EU868 maximum EIRP
    assert_eq!(config.nb_trans, 1);
}
