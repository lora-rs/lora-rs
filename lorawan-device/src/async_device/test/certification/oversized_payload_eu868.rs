use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::Uplink;

use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::types::DR;

use super::{build_mac, build_packet, util};

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn oversized_payload_sf12_bw_125_eu868() {
    let (radio, timer, mut async_device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Step 1: Send uplink, TCL responds with MAC commands
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });

    fn cfg_rx(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // LinkADRReq: DR=0 (SF12BW125), MAX, 0700, 01
        // RXParamSetupReq: Rx1DROffset=0, RX2DataRate=DR0 (SF12BW125), Frequency=869525000
        build_mac(buf, "03000700010500d2ad84", 1)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(cfg_rx).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    assert_eq!(device.mac.configuration.rx1_dr_offset, 0);
    assert_eq!(device.mac.configuration.rx2_data_rate, Some(DR::_0));
    assert_eq!(device.mac.configuration.rx2_frequency, Some(869525000));

    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [3, 7, 5, 7]);

    // Step 2: send uplink with response
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn oversized_payload(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_packet(buf, "07020101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101", 2)
    }
    timer.fire_most_recent().await;
    radio.handle_rxtx(oversized_payload).await;

    // We should skip this packet as it's oversized...
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    // Only RxParamSetupAns remains
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [5, 7]);

    // Step 3: send regular uplink with response
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    // Skip RX1
    timer.fire_most_recent().await;
    radio.handle_timeout().await;

    // Check that we are not using RX2 frequency
    let rx_conf = radio.get_rxconfig().await.unwrap();
    assert_ne!(rx_conf.rf.frequency, 869525000);

    timer.fire_most_recent().await;
    radio.handle_rxtx(oversized_payload).await;

    let rx_conf = radio.get_rxconfig().await.unwrap();
    assert_eq!(rx_conf.rf.frequency, 869525000);

    // RX2
    // We should skip this packet as it's oversized...
    let (_device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }
}
