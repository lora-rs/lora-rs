//! LoRaWAN 1.0.4 Certification testcases
//!
//! NewChannelReq for EU868 (2.5.2)
//! * Add/remove read-only default channels
//! * Add/remove single channel
//! * TODO: Invalid frequency/datarate tests
use super::{build_mac, util};
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::Uplink;

use lorawan::maccommands::parse_uplink_mac_commands;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
/// NewChannelReq: Add/remove read-only default channels
async fn newchannelreq_readonly_default_eu868() {
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

    // Attempt to add/modify read-only channels
    timer.fire_most_recent().await;
    fn tcl_1(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // NewChannelReq(ChIndex=0, Freq=867100000, DrRange=50)
        // NewChannelReq(ChIndex=1, Freq=867700000, DrRange=50)
        // NewChannelReq(ChIndex=2, Freq=867700000, DrRange=50)
        build_mac(buf, "0700184f84500701184f84500702184f8450", 1)
    }
    radio.handle_rxtx(tcl_1).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 3);
        assert_eq!(session.uplink.mac_commands(), [0x07, 0x00, 0x07, 0x00, 0x07, 0x00]);
    }

    // Step 2: send uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    // Attempt to remove read-only channels
    timer.fire_most_recent().await;
    fn tcl_2(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // NewChannelReq(ChIndex=0, Freq=0, DrRange=50)
        // NewChannelReq(ChIndex=1, Freq=0, DrRange=50)
        // NewChannelReq(ChIndex=2, Freq=0, DrRange=50)
        build_mac(buf, "070000000050070100000050070200000050", 2)
    }
    radio.handle_rxtx(tcl_2).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(2)) => {}
        _ => panic!(),
    }

    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 3);
        assert_eq!(session.uplink.mac_commands(), [0x07, 0x00, 0x07, 0x00, 0x07, 0x00]);
    }
}

#[tokio::test]
/// NewChannelReq: Addition/removal of single channel
async fn newchannelreq_e868() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Step 1: send uplink, TCL responds with LinkADRReq
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    fn tcl_1(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // LinkADRReq(...)
        build_mac(buf, "0350070001", 1)
    }
    radio.handle_rxtx(tcl_1).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(1)) => {}
        _ => panic!(),
    }

    // Step 2: send uplink, check whether DevStatusAns is present in MAC
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn tcl_2(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // NewChannelReq(ChIndex=0f, Freq=867700000, DrRange=50)
        build_mac(buf, "070f88668450", 2)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(tcl_2).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(2)) => {}
        _ => panic!(),
    }

    // Check whether channel was added
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 1);
        assert_eq!(session.uplink.mac_commands(), [0x07, 0x03]);
        let channel_mask = device.mac.region.channel_mask_get();
        assert_eq!(channel_mask.is_enabled(0xf), Ok(true))
    }

    // Step 3: send uplink, TCL responds with
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    fn tcl_3(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // NewChannelReq(ChIndex=0f, Freq=0, DrRange=50)
        build_mac(buf, "070f00000050", 3)
    }
    radio.handle_rxtx(tcl_3).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(3)) => {}
        _ => panic!(),
    }

    // Check whether channel was removed (channel disabled in ChannelMask)
    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 1);
        assert_eq!(session.uplink.mac_commands(), [0x07, 0x03]);
        let channel_mask = device.mac.region.channel_mask_get();
        assert_eq!(channel_mask.is_enabled(0xf), Ok(false))
    }
}
