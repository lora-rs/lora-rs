use super::*;
use crate::radio::{RfConfig, RxQuality, TxConfig};
use crate::region;
use lorawan::default_crypto::DefaultFactory;

mod timer;
use timer::{TestTimer, TimerChannel};

mod radio;
use radio::{RadioChannel, TestRadio, Uplink};

type Device =
    crate::async_device::Device<TestRadio, DefaultFactory, TestTimer, rand_core::OsRng, 512>;

pub fn get_key() -> [u8; 16] {
    [0; 16]
}
pub fn get_credentials() -> JoinMode {
    JoinMode::OTAA { deveui: [0; 8], appeui: [0; 8], appkey: get_key() }
}

fn setup() -> (RadioChannel, TimerChannel, Device) {
    let (radio_channel, mock_radio) = TestRadio::new();
    let (timer_channel, mock_timer) = TestTimer::new();
    let region = region::US915::default();
    let async_device: crate::async_device::Device<
        TestRadio,
        DefaultFactory,
        TestTimer,
        rand_core::OsRng,
        512,
    > = Device::new(region.into(), mock_radio, mock_timer, rand::rngs::OsRng);
    (radio_channel, timer_channel, async_device)
}

#[tokio::test]
async fn test_join_rx1() {
    let (radio, timer, mut async_device) = setup();
    // Run the device
    let async_device = tokio::spawn(async move { async_device.join(&get_credentials()).await });

    // Trigger beginning of RX1
    timer.fire().await;
    // Trigger handling of JoinAccept
    radio.handle_rxtx(handle_join_request);

    // Await the device to return and verify state
    if let Ok(()) = async_device.await.unwrap() {
        // NB: timer is armed twice (even if not fired twice)
        // because RX1 end is armed when packet is received
        assert_eq!(2, timer.get_armed_count().await);
    } else {
        assert!(false);
    }
}

#[tokio::test]
async fn test_join_rx2() {
    let (radio, timer, mut async_device) = setup();
    // Run the device
    let async_device = tokio::spawn(async move { async_device.join(&get_credentials()).await });

    // Trigger beginning of RX1
    timer.fire().await;
    // Trigger end of RX1
    timer.fire().await;
    // Trigger start of RX2
    timer.fire().await;
    // Pass the join request handler
    radio.handle_rxtx(handle_join_request);

    // Await the device to return and verify state
    if async_device.await.unwrap().is_ok() {
        assert_eq!(4, timer.get_armed_count().await);
    } else {
        assert!(false);
    }
}

#[tokio::test]
async fn test_no_join_accept() {
    let (_radio, timer, mut async_device) = setup();
    // Run the device
    let async_device = tokio::spawn(async move { async_device.join(&get_credentials()).await });

    // Trigger beginning of RX1
    timer.fire().await;
    // Trigger end of RX1
    timer.fire().await;
    // Trigger start of RX2
    timer.fire().await;
    // Trigger end of RX2
    timer.fire().await;

    // Await the device to return and verify state
    if let Err(Error::RxTimeout) = async_device.await.unwrap() {
        assert_eq!(4, timer.get_armed_count().await);
    } else {
        assert!(false);
    }
}

#[tokio::test]
async fn test_noise() {
    let (radio, timer, mut async_device) = setup();
    // Run the device
    let async_device = tokio::spawn(async move { async_device.join(&get_credentials()).await });
    // Trigger beginning of RX1
    timer.fire().await;
    // Send an invalid lorawan frame. 0 length is enough to confuse it
    radio.handle_rxtx(|_, _, _| 0);

    // Await the device to return and verify state
    if let Err(Error::UnableToDecodePayload(_)) = async_device.await.unwrap() {
        assert!(true);
    } else {
        assert!(false);
    }
}

/// Handle join request and pack a JoinAccept into RxBuffer
fn handle_join_request(uplink: Option<Uplink>, _config: RfConfig, rx_buffer: &mut [u8]) -> usize {
    //TODO: Verify the JoinRequest is ok
    if let Some(mut uplink) = uplink {
        if let PhyPayload::JoinRequest(_) = uplink.get_payload() {
            let mut buffer: [u8; 17] = [0; 17];
            let mut phy = lorawan::creator::JoinAcceptCreator::with_options(
                &mut buffer,
                DefaultFactory::default(),
            )
            .unwrap();
            let app_nonce_bytes = [1; 3];
            phy.set_app_nonce(&app_nonce_bytes);
            phy.set_net_id(&[1; 3]);
            phy.set_dev_addr(&[1; 4]);
            phy.build(&AES128(get_key())).unwrap();
            buffer.iter().zip(rx_buffer).for_each(|(a, b)| *b = *a);
            return 17;
        }
    }
    0
}
