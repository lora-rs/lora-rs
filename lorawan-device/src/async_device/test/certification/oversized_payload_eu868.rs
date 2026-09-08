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
        // LinkADRReq: DR=0 (SF12BW125), MAX, 0700, 02
        // RXParamSetupReq: Rx1DROffset=0, RX2DataRate=DR0 (SF12BW125), Frequency=869525000
        build_mac(buf, "03000700020500d2ad84", 1)
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
        build_packet(
            buf,
            "07020101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101",
            2,
        )
    }
    // The LinkADRReq in step 1 set NbTrans to 2, so the oversized (invalid,
    // therefore not acknowledging) downlink triggers a retransmission.
    timer.fire_when_armed(2).await; // RX1 start, attempt 1
    radio.handle_rxtx(oversized_payload).await; // oversized, dropped -> retransmit
    timer.fire_when_armed(3).await; // RX1 start, attempt 2
    radio.handle_timeout().await; // RX1 end
    timer.fire_when_armed(4).await; // RX2 start
    radio.handle_timeout().await; // RX2 end

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
    timer.fire_when_armed(5).await; // RX1 start, attempt 1
    radio.handle_timeout().await; // RX1 end

    // Check that we are not using RX2 frequency
    let rx_conf = radio.get_rxconfig().await.unwrap();
    assert_ne!(rx_conf.rf.frequency, 869525000);

    timer.fire_when_armed(6).await; // RX2 start, attempt 1
    radio.handle_rxtx(oversized_payload).await; // oversized, dropped -> retransmit

    let rx_conf = radio.get_rxconfig().await.unwrap();
    assert_eq!(rx_conf.rf.frequency, 869525000);

    // NbTrans is still 2, so the retransmission's windows time out and the
    // FCntUp is consumed
    timer.fire_when_armed(7).await; // RX1 start, attempt 2
    radio.handle_timeout().await; // RX1 end
    timer.fire_when_armed(8).await; // RX2 start
    radio.handle_timeout().await; // RX2 end

    // We should skip this packet as it's oversized...
    let (_device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }
}
