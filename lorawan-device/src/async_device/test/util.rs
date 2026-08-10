use crate::radio::RfConfig;
use lorawan::creator::DataFrame;
use lorawan::parser::{self, DataFrameType, DecryptedDataPayload, PhyPayload};

use super::{Device, get_dev_addr, get_key, radio::*, region, timer::*};
use crate::mac::Session;
pub(crate) use crate::test_util::{Uplink, get_crypto, handle_data_uplink_with_link_adr_req};
use crate::{AppSKey, NwkSKey};

fn default_session() -> Session {
    Session::new(NwkSKey::from(get_key()), AppSKey::from(get_key()), get_dev_addr())
}

pub fn session_with_region(region: region::Configuration) -> (RadioChannel, TimerChannel, Device) {
    let (radio_channel, mock_radio) = TestRadio::new();
    let (timer_channel, mock_timer) = TestTimer::new();
    let async_device = Device::new_with_session(
        region,
        mock_radio,
        mock_timer,
        rand::rngs::OsRng,
        Some(default_session()),
    );
    (radio_channel, timer_channel, async_device)
}

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
    setup_internal(Some(default_session()))
}

/// Handle an uplink and respond with two LinkAdrReq on Port 0
pub fn handle_class_c_uplink_after_join(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        let bytes = uplink.data_mut();
        let fcnt = match parser::parse(&*bytes) {
            Ok(PhyPayload::Data(data)) => {
                let fcnt = data.fhdr().fcnt() as u32;
                assert!(data.validate_mic(&get_crypto(), fcnt));
                fcnt
            }
            _ => panic!("Did not decode PhyPayload::Data!"),
        };
        let decrypted = DecryptedDataPayload::decrypt_in_place(
            bytes,
            Some(&get_crypto()),
            Some(&get_crypto()),
            fcnt,
        )
        .unwrap();
        assert_eq!(decrypted.fhdr().fcnt(), 0);
        // Respond with an empty downlink with the ack bit set
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedDown,
            dev_addr: get_dev_addr(),
            ack: true,
            ..Default::default()
        };
        let finished = frame.build_into(rx_buffer, &get_crypto(), Some(&get_crypto())).unwrap();
        finished.len()
    } else {
        panic!("No uplink passed to handle_class_c_uplink_after_join");
    }
}

#[cfg(feature = "class-c")]
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

    use super::SendResponse;
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
