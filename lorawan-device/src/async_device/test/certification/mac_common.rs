//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
//!
//! MAC testcases common for all regions:
//! * DevStatusReq (2.5.1)
//! * RXTimingSetupReq (2.5.5)
//! * LinkCheckReq (2.5.7)
//! * RxAppCnt (certification protocol)
//!
//! Region-specific tests (in separate files):
//! * NewChannelReq (2.5.2)
//! * DlChannelReq (2.5.3)
//! * RXParamSetupReq (2.5.4)
//!
//! TODO:
//! * TXParamSetupReq (2.5.6)
//! * LinkADRReq (2.5.8)
//! * DutyCycleReq (2.5.9)
//! * DeviceTimeReq (2.5.10)
use super::{decrypt_uplink, net, util};
use crate::test_util::Uplink;
use net::{ExchangeAction, Net, Pkt};

use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::parser::FrmPayload;

/// Check an uplink is a certification answer (FPort 224) with `payload`.
fn assert_cp_answer(uplink: &Uplink, payload: &[u8]) {
    let mut uplink = uplink.clone();
    let dl = decrypt_uplink(&mut uplink);
    assert_eq!(dl.f_port(), Some(224));
    assert_eq!(dl.frm_payload(), FrmPayload::Data(payload));
}

#[tokio::test]
/// 2.5.1. DevStatusReq test
/// Same scenario is used for all regions.
async fn eu868_devstatusreq_test() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.radio.set_snr(-15);
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: send uplink, TCL responds with MAC:DevStatusReq
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("06"))).await;
    r.expect_downlink_received();

    // TODO: Battery value is hardcoded to 255 in MAC for now
    let expected_ans = [0x06, 255, net.device().radio.snr_scaled()];

    // Check whether uplink has been populated with requested MAC:DevStatusAns command
    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, &expected_ans);

    // Step 2: send uplink, check whether DevStatusAns is present in MAC
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    let mut uplink = r.uplink(0).clone();
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), &expected_ans);
}

#[tokio::test]
/// 2.5.5. RxTimingSetup test
/// Same scenario is used for all regions.
async fn rxtimingsetup_eu868() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: send uplink, TCL responds with RXTimingSetupReq del=15
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("080F"))).await;
    r.expect_downlink_received();

    // Check whether uplink has been populated with the RXTimingSetupAns
    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [0x08]);

    // Step 2: send uplink, check whether response is present in MAC
    let r = net.exchange(&[1, 2, 3], 2, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    let mut uplink = r.uplink(0).clone();
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [0x08]);

    // The answer is repeated until a downlink arrives
    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [0x08]);

    // Step 3: trigger uplink with no data, TCL responds with
    // CP:EchoPayloadReq. The EchoIncPayloadAns is a regular uplink: it
    // opens its own RX windows.
    let r = net.exchange(&[], 2, false, ExchangeAction::rx1(Pkt::cp_cmd("08010203"))).await;
    r.expect_rx_complete();
    // The answer was transmitted inside the uplink's RX window
    assert_eq!(r.uplinks().len(), 2);
    assert_cp_answer(r.uplink(1), &[0x08, 0x02, 0x03, 0x04]);

    // Check that uplink has been cleared after receiving frame
    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 0);
}

#[tokio::test]
/// 2.5.7. LinkCheckReq test
/// Same scenario is used for all regions.
async fn eu868_linkcheckreq_test() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: send uplink, TCL responds with CP:LinkCheckReq
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::cp_cmd("20"))).await;
    // TODO: LinkCheckReq should be triggered automatically or not?
    r.expect_rx_complete();

    // Check whether uplink has been populated with requested MAC:LinkCheckReq command
    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, &[0x2]);

    // Step 2: trigger uplink with no data, TCL responds with MAC:LinkCheckAns
    let r = net.exchange(&[], 2, false, ExchangeAction::rx1(Pkt::mac("020301"))).await;
    r.expect_downlink_received();
    // Check whether the uplink contained the LinkCheckReq command
    let mut uplink = r.uplink(0).clone();
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [0x2]);

    // Step 3: Trigger empty uplink, TCL responds with FP:EchoPayloadReq
    let r = net.exchange(&[], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("08010203"))).await;
    r.expect_rx_complete();

    // Step 4: DUT automatically responded with FP:EchoPayloadAns
    assert_eq!(r.uplinks().len(), 2);
    assert_cp_answer(r.uplink(1), &[0x08, 0x02, 0x03, 0x04]);
}

#[tokio::test]
/// RxAppCnt test (certification protocol): the DUT increments RxAppCnt for
/// each applicative downlink, i.e. a frame with FPort > 0, plus an empty
/// downlink frame with the FCtrl ACK bit set (whether or not it carries a
/// FPort 0 MAC-command payload). Downlinks without application data and
/// without the ACK bit are not counted. The final RxAppCntAns reports the
/// number of counted downlinks, including the RxAppCntReq itself.
///
/// Runs in class C; the RxAppCntReq is delivered in the TX->RX1 gap,
/// so the answer is transmitted in place as a regular uplink.
async fn eu868_rxappcnt_test() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;

    // Downlink 1: FPort 3 without the ACK bit: application data, counted.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::app(3, &[1, 2, 3]))).await;
    r.expect_downlink_received();

    // Downlink 2: FPort 3 with the ACK bit: application data, counted.
    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::app(3, &[1, 2, 3]).ack()))
        .await;
    r.expect_downlink_received();

    // Downlink 3: FPort 0, empty MAC-command payload, no ACK bit:
    // not counted.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac(""))).await;
    r.expect_downlink_received();

    // Downlink 4: FPort 0, empty MAC-command payload, ACK bit:
    // counted as an applicative downlink.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("").ack())).await;
    r.expect_downlink_received();

    // Downlink 5: no FPort and no FRMPayload, no ACK bit: not counted.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::empty())).await;
    r.expect_downlink_received();

    // Downlink 6: no FPort and no FRMPayload, ACK bit: counted as an
    // applicative downlink.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::empty().ack())).await;
    r.expect_downlink_received();

    // Downlink 7: CP-CMD RxAppCntReq (FPort 224) in the TX->RX1 gap:
    // counted. The answer reports the number of counted downlinks so far,
    // i.e. 5 (downlinks 1, 2, 4, 6 and 7), and is transmitted in the gap;
    // RX1 and RX2 then time out.
    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::none().rxc_after(0, Pkt::cp_cmd("09")))
        .await;
    r.expect_rx_complete();
    assert_eq!(r.uplinks().len(), 2);
    assert_cp_answer(r.uplink(1), &[0x09, 0x05, 0x00]);

    // Final state: 5 applicative downlinks counted, and the answer
    // consumed exactly one FCntUp.
    let session = net.device().mac.get_session().unwrap();
    assert_eq!(session.rx_app_cnt, 5);
    assert_eq!(session.fcnt_up, 8);
}
