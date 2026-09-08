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
use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{Uplink, get_crypto, get_dev_addr};
use core::num::NonZeroU8;

use lorawan::creator::{DataFrame, Payload};
use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::parser::{DataFrameType, FrmPayload};

use std::sync::Arc;
use tokio::sync::Mutex;

use super::{build_mac, build_packet, decrypt_uplink};

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
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), &expected_ans);
}

#[tokio::test]
/// 2.5.5. RxTimingSetup test
/// Same scenario is used for all regions.
async fn rxtimingsetup_eu868() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Step 1: send uplink, TCL responds with CP:DevStatusReq
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    // RXTimingSetupReq del=15
    fn fp_rxtimingsetupreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_mac(buf, "080F", 1)
    }
    radio.handle_rxtx(fp_rxtimingsetupreq).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    // Check whether uplink has been populated with requested MAC:DevstatusAns command
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 1);
        assert_eq!(session.uplink.mac_commands(), [0x08]);
    }

    // Step 2: send uplink, check whether response is present in MAC
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    // RX1
    timer.fire_most_recent().await;
    radio.handle_timeout().await;

    // RX2
    timer.fire_most_recent().await;
    radio.handle_timeout().await;

    let (mut device, response) = task.await.unwrap();

    // Check whether sent uplink contained required DevStatusAns data
    let mut uplink = radio.get_last_uplink().await;
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [0x08]);

    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }

    // Check whether uplink still contains required data
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 1);
        assert_eq!(session.uplink.mac_commands(), [0x08]);
    }

    // Step 3: trigger uplink with no data
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn fp_echopayloadreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_packet(buf, "08010203", 2)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(fp_echopayloadreq).await;
    // The EchoIncPayloadAns is a regular uplink: it opens its own RX
    // windows.
    timer.fire_when_armed(5).await; // answer RX1 start
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end
    let (device, response) = task.await.unwrap();

    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!(),
    }

    // The answer was transmitted inside the uplink's RX window
    let mut uplink = radio.get_last_uplink().await;
    let dl = decrypt_uplink(&mut uplink);
    assert_eq!(dl.f_port(), Some(224));
    assert_eq!(dl.frm_payload(), FrmPayload::Data(&[0x08, 0x02, 0x03, 0x04]));

    // Check that uplink has been cleared after receiving frame
    // Check whether uplink still contains required data
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 0);
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
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [0x2]);

    // Step 3: Trigger empty uplink, TCL responds with FP:EchoPayloadReq
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn fp_echopayloadreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_packet(buf, "08010203", 3)
    }
    timer.fire_most_recent().await;
    radio.handle_rxtx(fp_echopayloadreq).await;
    // The EchoIncPayloadAns is a regular uplink: it opens its own RX
    // windows.
    timer.fire_when_armed(4).await; // answer RX1 start
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end
    let (_device, response) = task.await.unwrap();

    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!(),
    }

    // Step 4: DUT will automatically respond with FP:EchoPayloadAns
    let _complete = send_await_complete.clone();

    let mut uplink = radio.get_last_uplink().await;
    let dl = decrypt_uplink(&mut uplink);
    assert_eq!(dl.f_port(), Some(224));
    assert_eq!(dl.frm_payload(), FrmPayload::Data(&[0x08, 0x02, 0x03, 0x04]));
}

#[tokio::test]
/// RxAppCnt test (certification protocol): the DUT increments RxAppCnt for
/// each applicative downlink, i.e. a frame with FPort > 0, plus an empty
/// downlink frame with the FCtrl ACK bit set (whether or not it carries a
/// FPort 0 MAC-command payload). Downlinks without application data and
/// without the ACK bit are not counted. The final RxAppCntAns reports the
/// number of counted downlinks, including the RxAppCntReq itself.
async fn eu868_rxappcnt_test() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());

    /// Build a downlink of a given shape: `fport > 0` carries `data` on
    /// that port; `fport == 0` is a FPort-0 frame with an empty
    /// MAC-command payload; `fport == 0xFF` has neither a FPort nor a
    /// FRMPayload.
    fn build_downlink(buf: &mut [u8], fport: u8, ack: bool, fcnt: u16, data: &[u8]) -> usize {
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedDown,
            dev_addr: get_dev_addr(),
            ack,
            fcnt: fcnt.into(),
            payload: match fport {
                0xFF => Payload::None,
                0 => Payload::MacCommands(&[]),
                p => Payload::Data { f_port: NonZeroU8::new(p).unwrap(), data },
            },
            ..Default::default()
        };
        frame.build_into(buf, &get_crypto(), Some(&get_crypto())).unwrap().len()
    }

    // Downlink 1: FPort 3 without the ACK bit: application data, counted.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn dl_fport3_noack(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_downlink(buf, 3, false, 1, &[1, 2, 3])
    }
    radio.handle_rxtx(dl_fport3_noack).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Downlink 2: FPort 3 with the ACK bit: application data, counted.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn dl_fport3_ack(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_downlink(buf, 3, true, 2, &[1, 2, 3])
    }
    radio.handle_rxtx(dl_fport3_ack).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(2)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Downlink 3: FPort 0, empty MAC-command payload, no ACK bit:
    // not counted.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn dl_fport0_noack(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_downlink(buf, 0, false, 3, &[])
    }
    radio.handle_rxtx(dl_fport0_noack).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(3)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Downlink 4: FPort 0, empty MAC-command payload, ACK bit:
    // counted as an applicative downlink.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn dl_fport0_ack(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_downlink(buf, 0, true, 4, &[])
    }
    radio.handle_rxtx(dl_fport0_ack).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(4)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Downlink 5: no FPort and no FRMPayload, no ACK bit: not counted.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn dl_bare_noack(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_downlink(buf, 0xFF, false, 5, &[])
    }
    radio.handle_rxtx(dl_bare_noack).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(5)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Downlink 6: no FPort and no FRMPayload, ACK bit: counted as an
    // applicative downlink.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn dl_bare_ack(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_downlink(buf, 0xFF, true, 6, &[])
    }
    radio.handle_rxtx(dl_bare_ack).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(6)) => {}
        _ => panic!("expected DownlinkReceived, got {response:?}"),
    }

    // Downlink 7: CP-CMD RxAppCntReq (FPort 224): counted. The answer
    // reports the number of counted downlinks so far, i.e. 5 (downlinks 1,
    // 2, 4, 6 and 7).
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });
    timer.fire_most_recent().await; // RX1 start
    fn dl_rxappcntreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_downlink(buf, 224, false, 7, &[0x09])
    }
    radio.handle_rxtx(dl_rxappcntreq).await;
    // The answer is a regular uplink: it opens its own RX windows.
    timer.fire_when_armed(8).await; // answer RX1 start
    radio.handle_timeout().await; // answer RX1 end
    timer.fire_most_recent().await; // answer RX2 start
    radio.handle_timeout().await; // answer RX2 end
    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!("expected RxComplete, got {response:?}"),
    }

    let mut uplink = radio.get_last_uplink().await;
    let dl = decrypt_uplink(&mut uplink);
    assert_eq!(dl.f_port(), Some(224));
    assert_eq!(dl.frm_payload(), FrmPayload::Data(&[0x09, 0x05, 0x00]));

    // Final state: 5 applicative downlinks were counted.
    let session = device.mac.get_session().unwrap();
    assert_eq!(session.rx_app_cnt, 5);
}
