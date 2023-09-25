use super::{get_dev_addr, get_key, radio::*, region, timer::*, DefaultFactory, Device};

use crate::async_device::SessionData;
use crate::{AppSKey, NewSKey};

fn setup_internal(session_data: Option<SessionData>) -> (RadioChannel, TimerChannel, Device) {
    let (radio_channel, mock_radio) = TestRadio::new();
    let (timer_channel, mock_timer) = TestTimer::new();
    let region = region::US915::default();
    let async_device: crate::async_device::Device<
        TestRadio,
        DefaultFactory,
        TestTimer,
        rand_core::OsRng,
        512,
    > = Device::new_with_session(
        region.into(),
        mock_radio,
        mock_timer,
        rand::rngs::OsRng,
        session_data,
    );
    (radio_channel, timer_channel, async_device)
}

pub fn setup_with_session() -> (RadioChannel, TimerChannel, Device) {
    setup_internal(Some(SessionData {
        newskey: NewSKey::from(get_key()),
        appskey: AppSKey::from(get_key()),
        devaddr: get_dev_addr(),
        fcnt_up: 0,
        fcnt_down: 0,
    }))
}

pub fn setup() -> (RadioChannel, TimerChannel, Device) {
    setup_internal(None)
}
