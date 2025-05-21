use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{get_key, Uplink};
use lorawan::creator::DataPayloadCreator;
use lorawan::default_crypto::DefaultFactory;
use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::parser::{
    DataHeader, DataPayload, DecryptedDataPayload, EncryptedDataPayload, FRMPayload, PhyPayload,
};

use std::sync::Arc;
use tokio::sync::Mutex;

/// Decrypts the payload allowing access to payload contents
fn decrypt<T>(data: EncryptedDataPayload<T>, fcnt: u32) -> DecryptedDataPayload<T>
where
    T: AsMut<[u8]>,
    T: AsRef<[u8]>,
{
    data.decrypt(Some(&get_key().into()), Some(&get_key().into()), fcnt, &DefaultFactory).unwrap()
}

fn _build(buf: &mut [u8], payload_in_hex: &str, fcnt: u16, fport: u8) -> usize {
    let mut phy = DataPayloadCreator::new(buf).unwrap();
    phy.set_confirmed(false);
    phy.set_f_port(fport);
    phy.set_dev_addr(&[0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(fcnt.into());
    phy.set_fctrl(&lorawan::parser::FCtrl::new(0x20, true));
    let finished = match fport {
        0 => phy
            .build(
                &[],
                hex::decode(payload_in_hex).unwrap(),
                &get_key().into(),
                &get_key().into(),
                &DefaultFactory,
            )
            .unwrap(),
        _ => phy
            .build(
                &hex::decode(payload_in_hex).unwrap(),
                [],
                &get_key().into(),
                &get_key().into(),
                &DefaultFactory,
            )
            .unwrap(),
    };
    finished.len()
}

/// Build fport = 0 packet with MAC commands in fopts
fn build_mac(buf: &mut [u8], payload_in_hex: &str, fcnt: u16) -> usize {
    _build(buf, payload_in_hex, fcnt, 0)
}

/// Build certification protocol packet (fport = 224)
fn build_packet(buf: &mut [u8], payload_in_hex: &str, fcnt: u16) -> usize {
    _build(buf, payload_in_hex, fcnt, 224)
}

#[tokio::test]
async fn txframectrlreq_no_change() {
    // This test will check how TxFrameReqCtrlReq is handled and
    // whether it overrides confirmation flag for packets properly
    fn txframectrlreq_override_confirmed(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        // TxFrameReqCtrlReq([2]) - DUT should switch to Confirmed
        build_packet(buf, "0702", 1)
    }

    fn txframectrlreq_no_change(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        // TxFrameReqCtrlReq([0]) - no change
        build_packet(buf, "0700", 2)
    }

    // Note: This test is region-agnostic
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Check that override is not set
    if let Some(session) = device.mac.get_session() {
        assert_eq!(session.override_confirmed, None);
    }

    // Trigger uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    radio.handle_rxtx(txframectrlreq_override_confirmed).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }
    // Check that session is configured to override and send only confirmed packets
    if let Some(session) = device.mac.get_session() {
        assert_eq!(session.override_confirmed, Some(true));
    }

    // Trigger second uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    // TxFrameReqCtrl - no_change is no-op
    radio.handle_rxtx(txframectrlreq_no_change).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }
    // Check that override_confirm has not changed!
    if let Some(session) = device.mac.get_session() {
        assert_eq!(session.override_confirmed, Some(true));
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

    fn fp_echopayloadreq(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_packet(buf, "08010203", 3)
    }
    timer.fire_most_recent().await;
    radio.handle_rxtx(fp_echopayloadreq).await;
    let (_device, response) = task.await.unwrap();

    match response {
        Ok(SendResponse::RxComplete) => {}
        _ => panic!(),
    }

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
}
