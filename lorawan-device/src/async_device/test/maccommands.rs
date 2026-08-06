use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{get_key, Uplink};

use lorawan::default_crypto::DefaultFactory;
use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::types::ChannelMask;

use std::sync::Arc;
use tokio::sync::Mutex;

fn build_frm_payload(buf: &mut [u8], payload_in_hex: &str, fcnt: u32) -> usize {
    let mut phy = lorawan::creator::DataPayloadCreator::new(buf).unwrap();
    phy.set_confirmed(false);
    phy.set_f_port(0);
    phy.set_dev_addr([0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(fcnt);
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

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn linkadrreq_dynamic() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn add_disabled_channel(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // NewChannelReq - add new channel to slot 3
        // LinkADRReq - channelmask in bank = 1, mask = 0b111 (effectively disabling new channel)
        build_frm_payload(buf, "0703886684500350070001", 2)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(add_disabled_channel).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [7, 3, 3, 7]);

    // Trigger second uplink which calls "disable_all_channels" LinkADRReq and
    // our MAC layer effectively does it... This is wrong.
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn disable_all_channels(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // LinkADRReq - disable ALL channels in bank 0
        build_frm_payload(buf, "0350000001", 3)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(disable_all_channels).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn linkadrreq_fixed_125khz_extra_mask() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::US915::default().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn single_500_channel(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // LinkADRReq, SF8BW500, MAX, 0100, 71
        build_frm_payload(buf, "0340010071", 2)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(single_500_channel).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 7]);
    // Make sure that extra mask is properly applied to bank 8
    assert_eq!(
        device.mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap()
    );
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn linkaddreq_fixed_channel_mask_validation() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::US915::default().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn enable_125_disable_500(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // LinkADRReq, SF8BW500, MAX, 0000, 61
        build_frm_payload(buf, "0340000061", 2)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(enable_125_disable_500).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 4, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn disable_125_switch_to_500(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        // LinkADRReq, SF8BW500, MAX, 0100, 71
        build_frm_payload(buf, "0340010071", 3)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(disable_125_switch_to_500).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 7]);
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn linkaddreq_fixed_invalid_125khz() {
    // 2.5.8.b.ii.1.2. Invalid Command Processing
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::US915::default().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // step 1
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 1, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn step1(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_frm_payload(buf, "03000000710300ff0001", 1)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(step1).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [3, 7, 3, 7]);

    assert_eq!(
        device.mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0xff, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap()
    );

    // step 2
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 2, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn step2(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_frm_payload(buf, "0340000061", 2)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(step2).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);

    assert_eq!(
        device.mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0xff, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap()
    );

    // step 3
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn step3(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_frm_payload(buf, "0340010071", 3)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(step3).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 7]);

    assert_eq!(
        device.mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap()
    );

    // step 4
    // Invalid LinkADRReq enables one 125kHz channel with allowed 125kHz datarate
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 4, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn step4(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_frm_payload(buf, "0300000101", 4)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(step4).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);

    assert_eq!(
        device.mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap()
    );
}

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn linkadrreq_dynamic_invalid() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn addreq_chain(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // LinkADRReq, SF8BW125, 4, 0100, 00
        // LinkADRReq, SF9BW125, 1, 0000, 61
        // LinkADRReq, SF7BW125, 0, 0000, 01
        build_frm_payload(buf, "034401000003310000610350000001", 2)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(addreq_chain).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 3);
    assert_eq!(data, [3, 6, 3, 6, 3, 6]);
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
    build_frm_payload(buf, "0700184f84500701184f84500702184f8450", 2)
}

fn newchannelreq_invalid_eu868_dr(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    buf: &mut [u8],
) -> usize {
    // NewChannelReq with invalid DataRateRange
    build_frm_payload(buf, "0703287684cd", 2)
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
#[cfg(all(feature = "region-us915", feature = "experimental"))]
// TODO: Finalize RXParamSetupReq/RXTimingSetupReq
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
        phy.set_dev_addr([0; 4]);
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
        // TODO: Implement RxParamSetup and RxTimingSetup...
        // assert_eq!(data, [5, 7]);
    } else {
        panic!("Session not joined?");
    }
}
