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
use util::{setup, setup_with_session};

type Device =
    crate::async_device::Device<TestRadio, DefaultFactory, TestTimer, rand_core::OsRng, 512, 4>;

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
        // NB: timer is armed twice (even if not fired twice)
        // because RX1 end is armed when packet is received
        assert_eq!(2, timer.get_armed_count().await);
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
    timer.fire_most_recent().await;
    // Trigger start of RX2
    timer.fire_most_recent().await;
    // Pass the join request handler
    radio.handle_rxtx(handle_join_request::<4>).await;

    // Await the device to return and verify state
    if async_device.await.unwrap().is_ok() {
        assert_eq!(4, timer.get_armed_count().await);
    } else {
        panic!();
    }
}

#[tokio::test]
async fn test_no_join_accept() {
    let (_radio, timer, mut async_device) = setup();
    // Run the device
    let async_device =
        tokio::spawn(async move { async_device.join(&get_otaa_credentials()).await });

    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Trigger end of RX1
    timer.fire_most_recent().await;
    // Trigger start of RX2
    timer.fire_most_recent().await;
    // Trigger end of RX2
    timer.fire_most_recent().await;

    // Await the device to return and verify state
    let response = async_device.await.unwrap();
    if let Ok(JoinResponse::NoJoinAccept) = response {
        assert_eq!(4, timer.get_armed_count().await);
    } else {
        panic!("Unexpected response: {response:?}");
    }
}

#[tokio::test]
async fn test_noise() {
    let (radio, timer, mut async_device) = setup();
    // Run the device
    let async_device =
        tokio::spawn(async move { async_device.join(&get_otaa_credentials()).await });
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Send a bunch of invalid RF frames
    for _ in 0..12 {
        radio.handle_rxtx(|_, _, _| 5).await;
    }
    // Pass the join request handler
    radio.handle_rxtx(handle_join_request::<5>).await;
    // Await the device to return and verify state
    let Ok(JoinResponse::JoinSuccess) = async_device.await.unwrap() else {
        panic!();
    };
}

#[tokio::test]
async fn test_unconfirmed_uplink_no_downlink() {
    let (_radio, timer, mut async_device) = setup_with_session();
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
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger start of RX2
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger end of RX2
    timer.fire_most_recent().await;

    match async_device.await.unwrap() {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }
    assert!(*send_await_complete.lock().await);
}

#[tokio::test]
async fn test_confirmed_uplink_no_ack() {
    let (_radio, timer, mut async_device) = setup_with_session();
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
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger start of RX2
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger end of RX2
    timer.fire_most_recent().await;

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
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req).await;
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
    timer.fire_most_recent().await;
    assert!(!*send_await_complete.lock().await);
    // Trigger start of RX2
    timer.fire_most_recent().await;

    // Send a downlink confirmation
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req).await;

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
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req).await;
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
async fn test_join_rx1_class_c() {
    let (radio, timer, mut async_device) = setup();
    async_device.enable_class_c().await.unwrap();
    // Run the device
    let async_device =
        tokio::spawn(async move { async_device.join(&get_otaa_credentials()).await });

    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Trigger handling of JoinAccept
    radio.handle_rxtx(handle_join_request::<6>).await;
    // The stack internally sends a data frame after class C join
    // and we need to respond to it to "complete" the Join procedure

    // Trigger beginning of RX1
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    timer.fire_most_recent().await;

    radio.handle_rxtx(handle_class_c_uplink_after_join::<6>).await;

    // Await the device to return and verify state
    let response = async_device.await.unwrap();
    if let Ok(JoinResponse::JoinSuccess) = response {
        // async_device.await.unwrap() {
        // NB: timer is armed twice (even if not fired twice)
        // for each Join RX1 and Data RX1, because RX1 end
        // is armed when packet is received
        assert_eq!(4, timer.get_armed_count().await);
    } else {
        panic!();
    }
}

#[tokio::test]
async fn test_class_c_data_before_rx1() {
    let (radio, timer, mut async_device) = setup_with_session();
    async_device.enable_class_c().await.unwrap();
    // Run the device
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, true).await;
        (async_device, response)
    });

    // send first downlink before RX1
    radio.handle_rxtx(class_c_downlink).await;
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Send a downlink in RX1
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req).await;
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
    let (radio, timer, mut async_device) = setup_with_session();
    async_device.enable_class_c().await.unwrap();
    // Run the device
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, true).await;
        (async_device, response)
    });

    // send first downlink before RX1
    // Trigger beginning of RX1
    timer.fire_most_recent().await;
    // Trigger end of RX1
    timer.fire_most_recent().await;

    radio.handle_rxtx(class_c_downlink).await;
    // Send a downlink in RX2
    radio.handle_rxtx(handle_data_uplink_with_link_adr_req).await;
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
    let (radio, _timer, mut async_device) = setup_with_session();
    async_device.enable_class_c().await.unwrap();
    // Run the device
    let task = tokio::spawn(async move {
        let response = async_device.rxc_listen().await;
        (async_device, response)
    });

    radio.handle_rxtx(class_c_downlink).await;
    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(mac::Response::DownlinkReceived(_)) => (),
        _ => {
            panic!()
        }
    }
    let _ = device.take_downlink().unwrap();
}
