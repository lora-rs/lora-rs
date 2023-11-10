use super::{get_dev_addr, get_key, radio::*, region, timer::*, Device, SendResponse};

use crate::mac::Session;
use crate::test_util::handle_class_c_uplink_after_join;
use crate::{AppSKey, NewSKey};

fn setup_internal(session_data: Option<Session>) -> (RadioChannel, TimerChannel, Device) {
    let (radio_channel, mock_radio) = TestRadio::new();
    let (timer_channel, mock_timer) = TestTimer::new();
    let region = region::US915::default();
    let async_device = Device::new_with_session(
        region.into(),
        mock_radio,
        mock_timer,
        rand::rngs::OsRng,
        session_data,
    );
    (radio_channel, timer_channel, async_device)
}

pub fn setup_with_session() -> (RadioChannel, TimerChannel, Device) {
    setup_internal(Some(Session {
        newskey: NewSKey::from(get_key()),
        appskey: AppSKey::from(get_key()),
        devaddr: get_dev_addr(),
        fcnt_up: 0,
        fcnt_down: 0,
        confirmed: false,
        uplink: Default::default(),
    }))
}

pub async fn setup_with_session_class_c() -> (RadioChannel, TimerChannel, Device) {
    let (radio, timer, mut async_device) = setup_with_session();
    async_device.enable_class_c();
    // Run the device
    let task = tokio::spawn(async move {
        let response = async_device.send(&[3, 2, 1], 3, false).await;
        (async_device, response)
    });
    // timeout the first sends RX windows which enables class C
    timer.fire_most_recent().await;
    radio.handle_rxtx(handle_class_c_uplink_after_join).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(0)) => (),
        _ => {
            panic!()
        }
    }
    (radio, timer, device)
}

pub fn setup() -> (RadioChannel, TimerChannel, Device) {
    setup_internal(None)
}
