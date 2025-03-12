use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{get_key, Uplink};
use lorawan::creator::DataPayloadCreator;
use lorawan::default_crypto::DefaultFactory;

use std::sync::Arc;
use tokio::sync::Mutex;

fn build_packet(buf: &mut [u8], payload_in_hex: &str, fcnt: u16) -> usize {
    let mut phy = DataPayloadCreator::new(buf).unwrap();
    phy.set_confirmed(false);
    phy.set_f_port(224);
    phy.set_dev_addr(&[0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(fcnt.into());
    phy.set_fctrl(&lorawan::parser::FCtrl::new(0x20, true));
    let finished = phy
        .build(
            &hex::decode(payload_in_hex).unwrap(),
            [],
            &get_key().into(),
            &get_key().into(),
            &DefaultFactory,
        )
        .unwrap();
    finished.len()
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
