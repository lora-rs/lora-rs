use super::*;
use crate::{
    radio::{RfConfig, RxQuality, TxConfig},
    region,
    test_util::*,
};
use lorawan::default_crypto::DefaultFactory;
use std::sync::Arc;
use tokio::sync::Mutex;

mod timer;
use timer::TestTimer;

mod radio;
use radio::TestRadio;

mod util;
pub(crate) use util::{setup, setup_with_session};

#[cfg(feature = "certification")]
mod certification;

mod maccommands;

#[cfg(feature = "class-c")]
mod class_c;

#[cfg(feature = "multicast")]
mod multicast;

type Device = crate::async_device::Device<TestRadio, TestTimer, rand_core::OsRng, 512, 4>;

#[tokio::test]
async fn test_join_rx1() {
    let (radio, timer, mut async_device) = setup();
    // Run the device
    let async_device =
        tokio::spawn(async move { async_device.join(&get_otaa_credentials()).await });

    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Trigger handling of JoinAccept
    radio.handle_rxtx(handle_join_request::<3>).await;

    // Await the device to return and verify state
    if let Ok(JoinResponse::JoinSuccess) = async_device.await.unwrap() {
        assert_eq!(1, timer.get_armed_count().await);
    } else {
        panic!();
    }
}

#[tokio::test]
async fn test_join_rx2() {
    let (radio, timer, mut async_device) = setup();
    // Run the device
    let async_device =
        tokio::spawn(async move { async_device.join(&get_otaa_credentials()).await });

    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Trigger end of RX1
    radio.handle_timeout().await;
    // Trigger start of RX2
    timer.fire_most_recent().await;
    // Pass the join request handler
    radio.handle_rxtx(handle_join_request::<4>).await;

    // Await the device to return and verify state
    if async_device.await.unwrap().is_ok() {
        assert_eq!(2, timer.get_armed_count().await);
    } else {
        panic!();
    }
}

#[tokio::test]
async fn test_no_join_accept() {
    let (radio, timer, mut async_device) = setup();
    // Run the device
    let async_device =
        tokio::spawn(async move { async_device.join(&get_otaa_credentials()).await });

    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Trigger end of RX1
    radio.handle_timeout().await;
    // Trigger start of RX2
    timer.fire_most_recent().await;
    // Trigger end of RX2
    radio.handle_timeout().await;

    // Await the device to return and verify state
    let response = async_device.await.unwrap();
    if let Ok(JoinResponse::NoJoinAccept) = response {
        assert_eq!(2, timer.get_armed_count().await);
    } else {
        panic!("Unexpected response: {response:?}");
    }
}

#[tokio::test]
async fn test_unconfirmed_uplink_no_downlink() {
    let (radio, timer, mut async_device) = setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    // Run the device
    let complete = send_await_complete.clone();
    let async_device = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;

        let mut complete = complete.lock().await;
        *complete = true;
        response
    });
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger end of RX1
    radio.handle_timeout().await;
    // Trigger start of RX2
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger end of RX2
    radio.handle_timeout().await;

    match async_device.await.unwrap() {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }
    assert!(*send_await_complete.lock().await);
}

#[tokio::test]
async fn test_confirmed_uplink_no_ack() {
    let (radio, timer, mut async_device) = setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    // Run the device
    let complete = send_await_complete.clone();
    let async_device = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, true).await;

        let mut complete = complete.lock().await;
        *complete = true;
        response
    });
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger end of RX1
    radio.handle_timeout().await;
    // Trigger start of RX2
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger end of RX1
    radio.handle_timeout().await;

    match async_device.await.unwrap() {
        Ok(SendResponse::NoAck) => (),
        _ => panic!(),
    }
    assert!(*send_await_complete.lock().await);
}

#[tokio::test]
async fn test_confirmed_uplink_with_ack_rx1() {
    let (radio, timer, mut async_device) = setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    // Run the device
    let complete = send_await_complete.clone();
    let async_device = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, true).await;

        let mut complete = complete.lock().await;
        *complete = true;
        response
    });
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);

    // Send a downlink with confirmation
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req::<0, 0>).await;
    match async_device.await.unwrap() {
        Ok(SendResponse::DownlinkReceived(_)) => (),
        _ => {
            panic!()
        }
    }
}

#[tokio::test]
async fn test_confirmed_uplink_with_ack_rx2() {
    let (radio, timer, mut async_device) = setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    // Run the device
    let complete = send_await_complete.clone();
    let async_device = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, true).await;

        let mut complete = complete.lock().await;
        *complete = true;
        response
    });
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger end of RX1
    radio.handle_timeout().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger start of RX2
    timer.fire_most_recent().await;

    // Send a downlink confirmation
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req::<0, 0>).await;

    match async_device.await.unwrap() {
        Ok(SendResponse::DownlinkReceived(_)) => (),
        _ => {
            panic!()
        }
    }
}

#[tokio::test]
async fn test_link_adr_ans() {
    let (radio, timer, mut async_device) = setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    // Run the device
    let complete = send_await_complete.clone();
    let async_device = tokio::spawn(async move {
        async_device.send(&[1, 2, 3], 3, true).await.unwrap();
        {
            let mut complete = complete.lock().await;
            *complete = true;
        }
        async_device.send(&[1, 2, 3], 3, true).await
    });
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Send a downlink with confirmation
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req::<0, 0>).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
    assert!(*send_await_complete.lock().await);
    // at this point, the device thread should be sending the second frame
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Send a downlink with confirmation
    radio.handle_rxtx(handle_data_uplink_with_link_adr_ans).await;
    match async_device.await.unwrap() {
        Ok(SendResponse::DownlinkReceived(_)) => (),
        _ => {
            panic!()
        }
    }
}

#[tokio::test]
async fn invalid_maccommands_in_frmpayload() {
    fn packet_with_invalid_maccommand(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        rx_buffer: &mut [u8],
    ) -> usize {
        let mut phy = lorawan::creator::DataPayloadCreator::new(rx_buffer).unwrap();
        phy.set_confirmed(false);
        phy.set_f_port(0);
        phy.set_dev_addr([0; 4]);
        phy.set_uplink(false);
        phy.set_fcnt(16);
        phy.set_fctrl(&lorawan::parser::FCtrl::new(0x00, true));
        let finished = phy
            .build(&[], [3, 192, 0, 0, 0], &get_key().into(), &get_key().into(), &DefaultFactory)
            .unwrap();
        finished.len()
    }

    let (radio, timer, mut async_device) = util::setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    // Run the device listening for the setup message
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });
    // Handle reception in RX2
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    timer.fire_most_recent().await;
    radio.handle_rxtx(packet_with_invalid_maccommand).await;

    let (mut device, task) = task.await.unwrap();
    match task {
        Ok(SendResponse::DownlinkReceived(16)) => {
            // Nothing in downlink as expected
            assert!(device.take_downlink().is_none());
        }
        _ => {
            panic!()
        }
    }
}
