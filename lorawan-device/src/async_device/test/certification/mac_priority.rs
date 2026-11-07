//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
//!
//! MAC command prioritization
//!
//! Priority of including information in frame is following:
//! 1. MAC answers (highest priority)
//! 2. New MAC commands
//! 3. Application payload (lowest priority)
use super::{decrypt_uplink, net, util};
use lorawan::maccommands::{UplinkMacCommand, parse_uplink_mac_commands};
use lorawan::parser::FrmPayload;
use net::{ExchangeAction, Net, Pkt};

#[tokio::test]
/// 2.5.14. Multiple MAC commands prioritization
/// Steps 1-2: MAC command prioritization
async fn eu868_mac_priority() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: send uplink, TCL responds with:
    // CP-CMD LinkCheckReq
    // MAC-CMD DevStatusReq
    // MAC-CMD LinkADRReq(DataRate = Max125kHzDR)
    let downlink = Pkt::cp_cmd("20").fopts("060350070001");
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(downlink)).await;
    // TODO: LinkCheckReq should be triggered automatically or not?
    r.expect_rx_complete();

    // Check whether next uplink has been populated with requested MAC commands:
    // MAC-CMD DevStatusAns
    // MAC-CMD LinkADRAns
    // MAC-CMD LinkCheckReq
    let device = net.device();
    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 3);
    assert_eq!(data, &[0x06, 0xff, device.radio.snr_scaled(), 0x03, 0x07, 0x02]);

    // Step 2: trigger uplink with no data, TCL responds with MAC:LinkCheckAns
    let r = net.exchange(&[], 2, false, ExchangeAction::rx1(Pkt::mac("020301"))).await;
    r.expect_downlink_received();
}

#[tokio::test]
/// 2.5.14. Multiple MAC commands prioritization
/// Steps 3-6: MAC payload truncation
async fn eu868_mac_truncation() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 3: Trigger uplink, TCL responds with LinkADRReq(DataRate = Max125kHzDR)
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("0350070001"))).await;
    r.expect_downlink_received();

    // Step 4: Trigger uplink, TCL responds:
    // MAC-CMD1: DevStatusReq
    // MAC-CMD2: RxParamSetupReq
    // MAC-CMD3..N: DevStatusReq
    // MAC_CMD3..N+1: LinkADRReq(DataRate = Max125kHzDR)
    let downlink = Pkt::mac("060500d2ad840606060606060606060606060606060300070001");
    let r = net.exchange(&[1, 2, 3], 2, false, ExchangeAction::rx1(downlink)).await;
    r.expect_downlink_received();

    // All answers are queued. Truncation happens when the uplink is built.
    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 18);

    let r = net.exchange(&[2, 2, 3], 2, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    let mut uplink = r.uplink(0).clone();
    let dl = decrypt_uplink(&mut uplink);
    let FrmPayload::MacCommands(answers) = dl.frm_payload() else {
        panic!("expected MAC answers in FPort 0");
    };
    let (mut count, mut has_link_adr_ans, mut has_rx_param_setup_ans) = (0, false, false);
    for command in parse_uplink_mac_commands(answers) {
        count += 1;
        match command.unwrap() {
            UplinkMacCommand::LinkADRAns(..) => has_link_adr_ans = true,
            UplinkMacCommand::RXParamSetupAns(..) => has_rx_param_setup_ans = true,
            _ => {}
        }
    }
    assert_eq!(count, 17);
    assert!(!has_link_adr_ans, "LinkADRAns must be truncated");
    assert!(has_rx_param_setup_ans);
}

#[tokio::test]
async fn eu868_mac_non_truncated_payload() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 3: trigger an uplink and set DR5 (SF7BW125).
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("0350070001"))).await;
    r.expect_downlink_received();

    // Step 4: trigger an uplink and send many MAC commands in one block.
    let downlink = Pkt::mac("060500d2ad840606060606060606060606060606060350070001");
    let r = net.exchange(&[1, 2, 3], 2, false, ExchangeAction::rx1(downlink)).await;
    r.expect_downlink_received();

    // The next uplink must carry answers for both LinkADRReq and
    // RXParamSetupReq. They do not fit in FOpts together with all the
    // DevStatusAns answers, so they must not be truncated away.
    let r = net.exchange(&[], 3, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    let mut uplink = r.uplink(0).clone();
    let dl = decrypt_uplink(&mut uplink);
    let mut answers: heapless::Vec<u8, 256> = heapless::Vec::new();
    answers.extend_from_slice(dl.fhdr().f_opts()).unwrap();
    if let FrmPayload::MacCommands(data) = dl.frm_payload() {
        answers.extend_from_slice(data).unwrap();
    }

    let (mut has_link_adr_ans, mut has_rx_param_setup_ans) = (false, false);
    for command in parse_uplink_mac_commands(&answers) {
        match command.unwrap() {
            UplinkMacCommand::LinkADRAns(..) => has_link_adr_ans = true,
            UplinkMacCommand::RXParamSetupAns(..) => has_rx_param_setup_ans = true,
            _ => {}
        }
    }
    assert!(
        has_link_adr_ans && has_rx_param_setup_ans,
        "both LinkADRAns and RXParamSetupAns must be sent"
    );
}
