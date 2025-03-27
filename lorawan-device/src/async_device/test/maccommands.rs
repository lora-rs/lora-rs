use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{get_key, Uplink};

use lorawan::default_crypto::DefaultFactory;
use lorawan::maccommands::parse_uplink_mac_commands;

use std::sync::Arc;
use tokio::sync::Mutex;

fn build_frm_payload(buf: &mut [u8], payload_in_hex: &str) -> usize {
    let mut phy = lorawan::creator::DataPayloadCreator::new(buf).unwrap();
    phy.set_confirmed(false);
    phy.set_f_port(0);
    phy.set_dev_addr(&[0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(0xd);
    phy.set_fctrl(&lorawan::parser::FCtrl::new(0x20, true));
    let finished = phy
        .build(
            &[],
            hex::decode(payload_in_hex).unwrap(),
            &get_key().into(),
            &get_key().into(),
            &DefaultFactory,
        )
        .unwrap();
    finished.len()
}

fn newchannelreq_invalid_eu868(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    buf: &mut [u8],
) -> usize {
    // NewChannelReqPayload([0, 24, 79, 132, 80])
    // NewChannelReqPayload([1, 24, 79, 132, 80])
    // NewChannelReqPayload([2, 24, 79, 132, 80])
    // EU868 - first 3 channels are join channels and readonly
    build_frm_payload(buf, "0700184f84500701184f84500702184f8450")
}

fn newchannelreq_invalid_eu868_dr(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    buf: &mut [u8],
) -> usize {
    // NewChannelReq with invalid DataRateRange
    build_frm_payload(buf, "0703287684cd")
}

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn newchannelreq_eu868_readonly() {
    let (radio, timer, mut async_device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });

    timer.fire_most_recent().await;
    radio.handle_rxtx(newchannelreq_invalid_eu868).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 3);
        // For all 3 channels freq and dr are nacked (0b11)
        assert_eq!(data, [7, 0, 7, 0, 7, 0]);
    } else {
        panic!("Session not joined?");
    }
}

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn newchannelreq_eu868_invalid_dr() {
    let (radio, timer, mut async_device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });

    timer.fire_most_recent().await;
    radio.handle_rxtx(newchannelreq_invalid_eu868_dr).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 1);
        // Frequency is acked (bit 0), dr is invalid (bit 1)
        assert_eq!(data, [7, 0b01]);
    } else {
        panic!("Session not joined?");
    }
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn newchannelreq_fixed_region_ignore() {
    let (radio, timer, mut async_device) =
        util::session_with_region(crate::region::US915::default().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });

    timer.fire_most_recent().await;
    radio.handle_rxtx(newchannelreq_invalid_eu868).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        // Fixed channel region ignores NewChannelReq commands
        assert_eq!(parse_uplink_mac_commands(data).count(), 0);
    } else {
        panic!("Session not joined?");
    }
}

#[tokio::test]
// TODO: Finalize RXParamSetupReq
async fn maccommands_in_frmpayload() {
    fn frmpayload_maccommands(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        rx_buffer: &mut [u8],
    ) -> usize {
        // FRMPayload contains:
        // - DevStatusReq(..)
        // - RXParamSetupReq(RXParamSetupReqPayload([2, 210, 173, 132])) - freq: 869525000
        // - RXTimingSetupReq(RXTimingSetupReqPayload([1]))
        // - LinkADRReq(LinkADRReqPayload([80, 0, 0, 97]))
        let mut phy = lorawan::creator::DataPayloadCreator::new(rx_buffer).unwrap();
        phy.set_confirmed(false);
        phy.set_f_port(0);
        phy.set_dev_addr(&[0; 4]);
        phy.set_uplink(false);
        phy.set_fcnt(5);
        phy.set_fctrl(&lorawan::parser::FCtrl::new(0x00, true));
        let finished = phy
            .build(
                &[],
                [6, 5, 2, 0xd2, 0xad, 0x84, 8, 1, 3, 0x50, 0, 0, 0x61],
                &get_key().into(),
                &get_key().into(),
                &DefaultFactory,
            )
            .unwrap();
        finished.len()
    }

    let (radio, timer, mut async_device) = util::setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });

    // Handle reception in RX1
    timer.fire_most_recent().await;

    radio.handle_rxtx(frmpayload_maccommands).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(5)) => {}
        _ => panic!(),
    }

    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 4);
        // LinkADRReq sends freq = 869525000, but this is invalid in US915
        assert_eq!(device.mac.configuration.rx2_frequency, None);
    } else {
        panic!("Session not joined?");
    }
}
