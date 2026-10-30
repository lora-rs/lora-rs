//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
//!
//! MAC command prioritization
//!
//! Priority of including information in frame is following:
//! 1. MAC answers (highest priority)
//! 2. New MAC commands
//! 3. Application payload (lowest priority)
use super::{build_mac, decrypt_uplink, packet_with_mac, util};
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::Uplink;
use lorawan::maccommands::{UplinkMacCommand, parse_uplink_mac_commands};
use lorawan::parser::FrmPayload;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
/// 2.5.14. Multiple MAC commands prioritization
/// Steps 1-2: MAC command prioritization
async fn eu868_mac_priority() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Step 1: send uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    // .. TCL responds with:
    // CP-CMD LinkCheckReq
    // MAC-CMD DevStatusReq
    // MAC-CMD LinkADRReq(DataRate = Max125kHzDR)
    fn fp_linkcheckreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        packet_with_mac(buf, 224, "20", "060350070001", 1)
    }
    radio.handle_rxtx(fp_linkcheckreq).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        // TODO: LinkCheckReq should be triggered automatically or not?
        Ok(SendResponse::RxComplete) => {}
        _ => panic!(),
    }

    // Check whether next uplink has been populated with requested MAC commands:
    // MAC-CMD DevStatusAns
    // MAC-CMD LinkADRAns
    // MAC-CMD LinkCheckReq
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 3);
        assert_eq!(
            session.uplink.mac_commands(),
            &[0x06, 0xff, device.radio.snr_scaled(), 0x03, 0x07, 0x02]
        );
    }

    // Step 2: trigger uplink with no data, TCL responds with MAC:LinkCheckAns
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn tcl_mac_linkcheckans(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_mac(buf, "020301", 2)
    }
    timer.fire_most_recent().await;
    radio.handle_rxtx(tcl_mac_linkcheckans).await;
    let (_device, response) = task.await.unwrap();

    match response {
        Ok(SendResponse::DownlinkReceived(2)) => {}
        _ => panic!(),
    }
}

#[tokio::test]
/// 2.5.14. Multiple MAC commands prioritization
/// Steps 3-6: MAC payload truncation
async fn eu868_mac_truncation() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Step 3: Trigger uplink...
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    // ...TCL responds with LinkADRReq(DataRate = Max125kHzDR)
    timer.fire_most_recent().await;
    fn tcl_3(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_mac(buf, "0350070001", 1)
    }
    radio.handle_rxtx(tcl_3).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    // Step 4: Trigger uplink...
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;

    // ...TCL responds:
    // MAC-CMD1: DevStatusReq
    // MAC-CMD2: RxParamSetupReq
    // MAC-CMD3..N: DevStatusReq
    // MAC_CMD3..N+1: LinkADRReq(DataRate = Max125kHzDR)
    fn tcl_4(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_mac(buf, "060500d2ad840606060606060606060606060606060300070001", 2)
    }
    radio.handle_rxtx(tcl_4).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(2)) => {}
        _ => panic!(),
    }

    // Check that outgoing payload is truncated to 5 commands
    // LinkADRAns is not sent in the response as it must be truncated due to
    // payload size restrictions.
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 5);
    }

    let complete = send_await_complete.clone();

    let _task = tokio::spawn(async move {
        let response = device.send(&[2, 2, 3], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;

    let mut uplink = radio.get_last_uplink().await;
    let dl = decrypt_uplink(&mut uplink);
    assert_eq!(dl.frm_payload(), FrmPayload::Data(&[0x02, 0x02, 0x03]));
}

#[tokio::test]
#[should_panic(expected = "both LinkADRAns and RXParamSetupAns must be sent")]
async fn eu868_mac_non_truncated_payload() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());

    // Step 3: trigger an uplink and set DR5 (SF7BW125).
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        (device, response)
    });

    timer.fire_most_recent().await;
    fn tcl_3(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_mac(buf, "0350070001", 1)
    }
    radio.handle_rxtx(tcl_3).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    // Step 4: trigger an uplink and send many MAC commands in one block.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 2, false).await;
        (device, response)
    });

    timer.fire_most_recent().await;
    fn tcl_4(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_mac(buf, "060500d2ad840606060606060606060606060606060350070001", 2)
    }
    radio.handle_rxtx(tcl_4).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(2)) => {}
        _ => panic!(),
    }

    // The next uplink must carry answers for both LinkADRReq and
    // RXParamSetupReq. They do not fit in FOpts together with all the
    // DevStatusAns answers, so they must not be truncated away.
    let task = tokio::spawn(async move {
        let response = device.send(&[], 3, false).await;
        (device, response)
    });

    timer.fire_most_recent().await;
    let mut uplink = radio.get_last_uplink().await;
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
    task.abort();
    assert!(
        has_link_adr_ans && has_rx_param_setup_ans,
        "both LinkADRAns and RXParamSetupAns must be sent"
    );
}
