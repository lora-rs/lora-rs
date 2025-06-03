//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
//!
//! DlChannelReq for EU868 region
use super::{build_mac, util};
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::Uplink;

use lorawan::parser::{DataHeader, DataPayload, PhyPayload};

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
/// 2.5.3. DlChannelReq test for EU868
/// TODO: Implement test which checks for RX1 frequency
async fn eu868_dlchannelreq() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Step 1: send uplink, TCL responds with MAC:DlChannelReq
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    fn tcl_1(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // DlChannelReq(channel_index=0, frequency=868300000)
        build_mac(buf, "0a00f87d84", 1)
    }
    radio.handle_rxtx(tcl_1).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    // Step 2: send uplink, TCL ignores it...
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 4, false).await;
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
    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }

    assert!(*send_await_complete.lock().await);
    // Check that our mac response was present
    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.fhdr().data(), [0x0a, 0x03]);
        }
        _ => panic!(),
    }

    // Step 3: send uplink, TCL ignores it..
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 4, false).await;
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

    let (_device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }

    assert!(*send_await_complete.lock().await);
    // Check that our mac response was present
    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.fhdr().data(), [0x0a, 0x03]);
        }
        _ => panic!(),
    }
}
