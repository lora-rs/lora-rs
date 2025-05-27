//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
//!
//! MAC testcases common for all regions:
//! * DevStatusReq (2.5.1)
//! * LinkCheckReq (2.5.7)
//!
//! TODO:
//! * NewChannelReq (2.5.2)
//! * DlChannelReq (2.5.3)
//! * RXParamSetupReq (2.5.4)
//! * RXTimingSetupReq (2.5.5)
//! * TXParamSetupReq (2.5.6)
//! * LinkADRReq (2.5.8)
//! * DutyCycleReq (2.5.9)
//! * DeviceTimeReq (2.5.10)
use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::Uplink;

use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::parser::{DataHeader, DataPayload, FRMPayload, PhyPayload};

use std::sync::Arc;
use tokio::sync::Mutex;

use super::{build_mac, build_packet, decrypt};

#[tokio::test]
/// 2.5.1. DevStatusReq test
/// Same scenario is used for all regions.
async fn eu868_devstatusreq_test() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    device.radio.set_snr(-15);

    // Step 1: send uplink, TCL responds with CP:DevStatusReq
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    fn fp_devstatusreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_mac(buf, "06", 1)
    }
    radio.handle_rxtx(fp_devstatusreq).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    // TODO: Battery value is hardcoded to 255 in MAC for now
    let expected_ans = [0x06, 255, device.radio.snr_scaled()];

    // Check whether uplink has been populated with requested MAC:DevstatusAns command
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 1);
        assert_eq!(session.uplink.mac_commands(), &expected_ans);
    }

    // Step 2: send uplink, check whether DevStatusAns is present in MAC
    let complete = send_await_complete.clone();
    let _task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });
    timer.fire_most_recent().await;

    // Check whether sent uplink contained required DevStatusAns data
    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.fhdr().data(), &expected_ans)
        }
        _ => panic!(),
    }
}

#[tokio::test]
/// 2.5.7. LinkCheckReq test
/// Same scenario is used for all regions.
async fn eu868_linkcheckreq_test() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Step 1: send uplink, TCL responds with CP:LinkCheckReq
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    fn fp_linkcheckreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_packet(buf, "20", 1)
    }
    radio.handle_rxtx(fp_linkcheckreq).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        // TODO: LinkCheckReq should be triggered automatically or not?
        Ok(SendResponse::RxComplete) => {}
        _ => panic!(),
    }

    // Check whether uplink has been populated with requested MAC:LinkCheckReq command
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 1);
        assert_eq!(session.uplink.mac_commands(), &[0x2]);
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
    let (mut device, response) = task.await.unwrap();

    match response {
        Ok(SendResponse::DownlinkReceived(2)) => {}
        _ => panic!(),
    }

    // Check whether previous uplink contains required LinkCheckReq command
    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.fhdr().data(), [0x2]);
        }
        _ => panic!(),
    }

    // Step 3: Trigger empty uplink, TCL responds with FP:EchoPayloadReq
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn verify_response(uplink: Option<Uplink>, _config: RfConfig, _rx_buffer: &mut [u8]) -> usize {
        println!("XXX: {:?}", uplink);
        0
    }

    fn fp_echopayloadreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_packet(buf, "08010203", 3)
    }
    timer.fire_most_recent().await;
    println!("RX: about to receive echo..");
    radio.handle_rxtx(fp_echopayloadreq).await;

    println!("RX: about to send out response..");
    radio.handle_rxtx(verify_response).await;

    timer.fire_most_recent().await;

    println!("???");

    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.f_port(), Some(224));
            let dl = decrypt(data, 3);
            assert_eq!(dl.frm_payload(), FRMPayload::Data(&[0x08, 0x02, 0x03, 0x04]));
        }
        _ => panic!(),
    }

    println!(" --> X");
    let (device, response) = task.await.unwrap();
    println!("Y");
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!(),
    }

    /*

    // Step 4: DUT will automatically respond with FP:EchoPayloadAns
    let _complete = send_await_complete.clone();

    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.f_port(), Some(224));
            let dl = decrypt(data, 3);
            assert_eq!(dl.frm_payload(), FRMPayload::Data(&[0x08, 0x02, 0x03, 0x04]));
        }
        _ => panic!(),
    }
    */
}
