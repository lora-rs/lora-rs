use super::util;
use crate::async_device::{ListenResponse, SendResponse};
use crate::radio::RfConfig;
use crate::test_util::{get_dev_addr, get_key, Uplink};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::default_crypto::DefaultFactory;
use lorawan::parser::DataFrameType;

pub fn class_c_downlink<const FCNT_DOWN: u32>(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: get_dev_addr(),
        fcnt: FCNT_DOWN,
        payload: Payload::Data { f_port: NonZeroU8::new(3).unwrap(), data: &[1, 2, 3] },
        ..Default::default()
    };
    let finished = frame
        .build_into(rx_buffer, &get_key().into(), Some(&get_key().into()), &DefaultFactory)
        .unwrap();
    finished.len()
}
#[tokio::test]
async fn test_class_c_data_before_rx1() {
    let (radio, timer, mut async_device) = util::setup_with_session_class_c().await;
    // Run the device
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, true).await;
        (async_device, response)
    });

    // send first downlink before RX1
    radio.handle_rxtx(class_c_downlink::<1>).await;
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // We expect FCntUp 1 up since the test util for Class C setup sends first frame
    // We set FcntDown to 2, since ACK to setup (1) and Class C downlink above (2)
    radio.handle_rxtx(util::handle_data_uplink_with_link_adr_req::<1, 2>).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => (),
        _ => {
            panic!()
        }
    }
    let _ = device.take_downlink().unwrap();
    let _ = device.take_downlink().unwrap();
}

#[tokio::test]
async fn test_class_c_data_before_rx2() {
    let (radio, timer, mut async_device) = util::setup_with_session_class_c().await;
    // Run the device
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, true).await;
        (async_device, response)
    });

    // send first downlink before RX1
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Trigger end of RX1
    radio.handle_timeout().await;

    radio.handle_rxtx(class_c_downlink::<1>).await;
    // Trigger beginning of RX2
    timer.fire_most_recent().await;
    // We expect FCntUp 1 up since the test util for Class C setup sends first frame
    // We set FcntDown to 2, since ACK to setup (1) and Class C downlink above (2)
    radio.handle_rxtx(util::handle_data_uplink_with_link_adr_req::<1, 2>).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => (),
        _ => {
            panic!()
        }
    }
    let _ = device.take_downlink().unwrap();
    let _ = device.take_downlink().unwrap();
}

#[tokio::test]
async fn test_class_c_async_down() {
    let (radio, _timer, mut async_device) = util::setup_with_session_class_c().await;
    // Run the device
    let task = tokio::spawn(async move {
        let response = async_device.rxc_listen().await;
        (async_device, response)
    });

    radio.handle_rxtx(class_c_downlink::<1>).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(ListenResponse::DownlinkReceived(_)) => (),
        _ => {
            panic!()
        }
    }
    let _ = device.take_downlink().unwrap();
}
