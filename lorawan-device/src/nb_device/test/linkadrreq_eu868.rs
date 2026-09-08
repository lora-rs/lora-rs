//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.4
//!
//! EU868 LinkADRReq tests

use super::util::TestRadio;
use crate::nb_device::radio::Event as RadioEvent;
use crate::nb_device::{Device, Event, Response};
use crate::radio::RfConfig;
use crate::region::{Configuration, Region};
use crate::test_util::{RxTxHandler, Uplink, get_abp_credentials, get_crypto, get_dev_addr};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::parser::{self, DataFrameType, DecryptedDataPayload, FrmPayload, PhyPayload};

type TestDevice = Device<TestRadio, rand::rngs::OsRng, 255>;

/// New EU868 device with the shared test ABP credentials.
fn eu868_device() -> TestDevice {
    let mut device =
        Device::new(Configuration::new(Region::EU868), TestRadio::default(), rand::rngs::OsRng);
    device.join(get_abp_credentials()).unwrap();
    device
}

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

/// Parses the uplink, checks the MIC, and decrypts it in place, allowing
/// access to payload contents
fn decrypt_uplink(uplink: &mut Uplink) -> DecryptedDataPayload<'_> {
    let bytes = uplink.data_mut();
    let fcnt = match parser::parse(&*bytes) {
        Ok(PhyPayload::Data(data)) => {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_crypto(), fcnt));
            fcnt
        }
        _ => panic!("expected a data frame"),
    };
    DecryptedDataPayload::decrypt_in_place(bytes, Some(&get_crypto()), Some(&get_crypto()), fcnt)
        .unwrap()
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

/// Fire the RX1/RX2 window timeouts of the current transmission and return
/// the response to the last timeout (RX2 end): a completion response
/// (RxComplete, NoAck, ...), or a TimeoutRequest when the frame is
/// retransmitted.
fn expire_rx_windows(device: &mut TestDevice) -> Response {
    let mut response = device.handle_event(Event::TimeoutFired).unwrap();
    for _ in 0..3 {
        if !matches!(response, Response::TimeoutRequest(_)) {
            return response;
        }
        response = device.handle_event(Event::TimeoutFired).unwrap();
    }
    response
}

/// Fire the RX1 start timeout, then deliver a downlink in RX1 via the
/// handler; returns the response to the downlink.
fn deliver_downlink_rx1(device: &mut TestDevice, handler: RxTxHandler) -> Response {
    let _ = device.handle_event(Event::TimeoutFired).unwrap();
    device.get_radio().set_rxtx_handler(handler);
    device.handle_event(Event::RadioEvent(RadioEvent::Phy(()))).unwrap()
}

/// Fire timeouts until RX2 is active, then deliver a downlink in RX2 via the
/// handler; returns the response to the downlink.
fn deliver_downlink_rx2(device: &mut TestDevice, handler: RxTxHandler) -> Response {
    let _ = device.handle_event(Event::TimeoutFired).unwrap();
    let _ = device.handle_event(Event::TimeoutFired).unwrap();
    let _ = device.handle_event(Event::TimeoutFired).unwrap();
    device.get_radio().set_rxtx_handler(handler);
    device.handle_event(Event::RadioEvent(RadioEvent::Phy(()))).unwrap()
}

#[test]
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
fn eu868_linkadrreq_redundancy() {
    let mut device = eu868_device();

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

    // Step 1: DUT sends an unconfirmed frame (FCntUp = 0); the TCL responds
    // on RX1 with CP-CMD RxAppCntReq. The answer (FCntUp = 1) opens its own
    // RX windows.
    let response = device.send(&[1, 2, 3], 3, false).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    fn tcl_rxappcnt_req_1(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 0);
        rxappcnt_req(buf, 0)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_rxappcnt_req_1);
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = expire_rx_windows(&mut device);
    assert!(matches!(response, Response::RxComplete));
    let uplink = device.get_radio().take_last_uplink().unwrap();
    assert_uplink(&uplink, 1, false, false, &[], Some(224), &[0x09, 0x01, 0x00]);

    let x: u16 = 1;

    // Step 3: DUT sends an unconfirmed frame (FCntUp = 2); the TCL responds
    // with MAC-CMD LinkADRReq, NbTrans = 2.
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_linkadr_req_nbtrans2(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 2);
        linkadr_req(buf, 2, 1)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_linkadr_req_nbtrans2);
    assert!(matches!(response, Response::DownlinkReceived(1)));
    // The next uplink must carry LinkADRAns (all settings valid)
    let session = device.shared.mac.get_session().unwrap();
    assert_eq!(session.uplink.mac_commands(), LINKADR_ANS);

    // Steps 4-5: the uplink (FCntUp = 3) carries the LinkADRAns and, as it
    // receives no downlink, is retransmitted once (NbTrans = 2) with the
    // same FCntUp.
    device.send(&[1, 2, 3], 3, false).unwrap();
    let tx1 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // RX2 end -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let tx2 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // FCntUp consumed
    assert!(matches!(response, Response::RxComplete));
    assert_data_uplink(&tx1, 3, false, false, LINKADR_ANS);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(tx1.data(), tx2.data());

    // Steps 6-7: next uplink (FCntUp = 4) is also sent twice (NbTrans = 2 is
    // still in effect).
    device.send(&[1, 2, 3], 3, false).unwrap();
    let tx1 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let tx2 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // -> FCntUp consumed
    assert!(matches!(response, Response::RxComplete));
    assert_data_uplink(&tx1, 4, false, false, &[]);
    assert_eq!(tx1.data(), tx2.data());

    // Step 8: DUT uplink (FCntUp = 5); the TCL responds with RxAppCntReq on
    // the RX1 window. Step 9: the RxAppCntAns uplink (FCntUp = 6) reports
    // RxAppCnt = x + 1; it is retransmitted once (NbTrans = 2).
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_rxappcnt_req_rx1(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 5);
        rxappcnt_req(buf, 2)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_rxappcnt_req_rx1);
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let tx1 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let tx2 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // -> FCntUp consumed
    assert!(matches!(response, Response::RxComplete));
    assert_uplink(&tx1, 6, false, false, &[], Some(224), &[0x09, (x + 1) as u8, 0x00]);
    assert_eq!(tx1.data(), tx2.data());

    // Step 11: DUT uplink (FCntUp = 7); the TCL responds with RxAppCntReq on
    // the RX2 window. Step 12: the RxAppCntAns uplink (FCntUp = 8) reports
    // RxAppCnt = x + 2; it is retransmitted once (NbTrans = 2).
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_rxappcnt_req_rx2(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 7);
        rxappcnt_req(buf, 3)
    }
    let response = deliver_downlink_rx2(&mut device, tcl_rxappcnt_req_rx2);
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let tx1 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let tx2 = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device); // -> FCntUp consumed
    assert!(matches!(response, Response::RxComplete));
    assert_uplink(&tx1, 8, false, false, &[], Some(224), &[0x09, (x + 2) as u8, 0x00]);
    assert_eq!(tx1.data(), tx2.data());

    // Step 12 (cont'd) / 13: the TCL sends a LinkADRReq with NbTrans = 1.
    // The answer's RX windows above are kept silent, so the TCL delivers it
    // in the next regular uplink's window (FCntUp = 9) instead.
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_linkadr_req_nbtrans1(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 9);
        linkadr_req(buf, 1, 4)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_linkadr_req_nbtrans1);
    assert!(matches!(response, Response::DownlinkReceived(4)));

    // Step 14: the next uplink (FCntUp = 10) carries the LinkADRAns ("uplink
    // sent once", NbTrans = 1) and the TCL responds with CP-CMD
    // TxFramesCtrlReq (frame type = Confirmed).
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_txframectrl_confirmed(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        let uplink = uplink.expect("expected an uplink");
        assert_data_uplink(&uplink, 10, false, false, LINKADR_ANS);
        build_downlink(buf, DataFrameType::UnconfirmedDown, true, 224, "0702", 5)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_txframectrl_confirmed);
    assert!(matches!(response, Response::DownlinkReceived(5)));
    // The session is now configured to send only confirmed frames
    let session = device.shared.mac.get_session().unwrap();
    assert_eq!(session.override_confirmed, Some(true));

    // Step 15: the DUT sends a confirmed frame (FCntUp = 11) although the
    // application asked for unconfirmed; the TCL acknowledges it and responds
    // (confirmed downlink) with a LinkADRReq, NbTrans = 3.
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_linkadr_req_nbtrans3(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        let uplink = uplink.expect("expected an uplink");
        assert_data_uplink(&uplink, 11, true, false, &[]);
        let payload = format!("{LINKADR_REQ}03");
        build_downlink(buf, DataFrameType::ConfirmedDown, true, 0, &payload, 6)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_linkadr_req_nbtrans3);
    assert!(matches!(response, Response::DownlinkReceived(6)));

    // Steps 16-18: the next confirmed uplink (FCntUp = 12) carries the
    // LinkADRAns and, as it is never acknowledged, is sent three times
    // (NbTrans = 3) with the same FCntUp.
    device.send(&[1, 2, 3], 3, false).unwrap();
    let mut transmissions = vec![device.get_radio().take_last_uplink().unwrap()];
    let response = expire_rx_windows(&mut device); // -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    transmissions.push(device.get_radio().take_last_uplink().unwrap());
    let response = expire_rx_windows(&mut device); // -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    transmissions.push(device.get_radio().take_last_uplink().unwrap());
    let response = expire_rx_windows(&mut device); // -> FCntUp consumed
    assert!(matches!(response, Response::NoAck));
    for tx in &transmissions {
        assert_data_uplink(tx, 12, true, true, LINKADR_ANS);
    }
    let bytes: Vec<Vec<u8>> = transmissions.iter().map(|tx| tx.data().to_vec()).collect();
    assert_eq!(bytes[0], bytes[1]);
    assert_eq!(bytes[1], bytes[2]);

    // Step 19: the next uplink (FCntUp = 13) is still confirmed (the override
    // is in effect until told otherwise); the TCL acknowledges the previous
    // frame and responds with CP-CMD TxFramesCtrlReq (frame type =
    // Unconfirmed) to revert.
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_txframectrl_unconfirmed(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        let uplink = uplink.expect("expected an uplink");
        assert_data_uplink(&uplink, 13, true, false, &[]);
        build_downlink(buf, DataFrameType::UnconfirmedDown, true, 224, "0701", 7)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_txframectrl_unconfirmed);
    assert!(matches!(response, Response::DownlinkReceived(7)));
    let session = device.shared.mac.get_session().unwrap();
    assert_eq!(session.override_confirmed, Some(false));

    // Step 20: the next uplink (FCntUp = 14) is unconfirmed again; the TCL
    // responds with RxAppCntReq. Steps 21-23: the RxAppCntAns uplink
    // (FCntUp = 15) reports the number of applicative downlinks the DUT has
    // received: the three RxAppCntReqs, the two TxFramesCtrlReqs, plus the
    // step-15 downlink, which carries no application data but acknowledges
    // the previous confirmed uplink (an empty downlink frame with the ACK
    // bit set is an applicative downlink per the certification spec), i.e.
    // x + 6. The answer is sent three times (NbTrans = 3) with the same
    // FCntUp.
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_rxappcnt_req_4(uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        expect_uplink_fcnt(uplink, 14);
        rxappcnt_req(buf, 8)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_rxappcnt_req_4);
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let mut transmissions = vec![device.get_radio().take_last_uplink().unwrap()];
    let response = expire_rx_windows(&mut device); // -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    transmissions.push(device.get_radio().take_last_uplink().unwrap());
    let response = expire_rx_windows(&mut device); // -> retransmit
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    transmissions.push(device.get_radio().take_last_uplink().unwrap());
    let response = expire_rx_windows(&mut device); // -> FCntUp consumed
    assert!(matches!(response, Response::RxComplete));
    for tx in &transmissions {
        assert_uplink(tx, 15, false, false, &[], Some(224), &[0x09, (x + 6) as u8, 0x00]);
    }

    // Step 23 (cont'd) / 24: the TCL sends a LinkADRReq with NbTrans = 0,
    // which means the default of 1 transmission, in the next regular
    // uplink's window (FCntUp = 16).
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_linkadr_req_nbtrans0(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 16);
        linkadr_req(buf, 0, 9)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_linkadr_req_nbtrans0);
    assert!(matches!(response, Response::DownlinkReceived(9)));

    // Step 24: the uplink carrying the LinkADRAns (FCntUp = 17) is sent only
    // once.
    device.send(&[1, 2, 3], 3, false).unwrap();
    let tx = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device);
    assert!(matches!(response, Response::RxComplete));
    assert_data_uplink(&tx, 17, false, false, LINKADR_ANS);

    // Step 25: DUT uplink (FCntUp = 18); the TCL responds with a LinkADRReq
    // with NbTrans = 1.
    device.send(&[1, 2, 3], 3, false).unwrap();
    fn tcl_linkadr_req_nbtrans1_final(
        uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        expect_uplink_fcnt(uplink, 18);
        linkadr_req(buf, 1, 10)
    }
    let response = deliver_downlink_rx1(&mut device, tcl_linkadr_req_nbtrans1_final);
    assert!(matches!(response, Response::DownlinkReceived(10)));

    // Step 26: the uplink (FCntUp = 19) carries the LinkADRAns; the DUT has
    // reverted to the default settings.
    device.send(&[1, 2, 3], 3, false).unwrap();
    let tx = device.get_radio().take_last_uplink().unwrap();
    let response = expire_rx_windows(&mut device);
    assert!(matches!(response, Response::RxComplete));
    assert_data_uplink(&tx, 19, false, false, LINKADR_ANS);

    // Final state: all 20 FCntUps (0 .. 19) were consumed, the DUT received
    // 7 applicative downlinks (six FPort > 0, plus the step-15 downlink
    // without application data carrying the ACK bit), the confirmed override
    // is off and the LinkADR settings hold DR5 / maximum power / NbTrans = 1.
    let session = device.shared.mac.get_session().unwrap();
    assert_eq!(session.fcnt_up, 20);
    assert_eq!(session.rx_app_cnt, 7);
    assert_eq!(session.override_confirmed, Some(false));
    let config = &device.shared.mac.configuration;
    assert_eq!(config.data_rate, crate::region::DR::_5);
    assert_eq!(config.tx_power, Some(16)); // EU868 maximum EIRP
    assert_eq!(config.nb_trans, 1);
}
