use crate::radio::RfConfig;
use lorawan::creator::DataPayloadCreator;
use lorawan::default_crypto::DefaultFactory;
use lorawan::parser::{DataHeader, DataPayload, FCtrl, PhyPayload};

use super::{get_dev_addr, get_key, radio::*, region, timer::*, Device};
use crate::mac::Session;
pub(crate) use crate::test_util::{handle_data_uplink_with_link_adr_req, Uplink};
use crate::{AppSKey, NwkSKey};

fn default_session() -> Session {
    Session {
        nwkskey: NwkSKey::from(get_key()),
        appskey: AppSKey::from(get_key()),
        devaddr: get_dev_addr(),
        fcnt_up: 0,
        fcnt_down: 0,
        confirmed: false,
        uplink: Default::default(),
        #[cfg(feature = "certification")]
        override_adr: false,
        #[cfg(feature = "certification")]
        override_confirmed: None,
        #[cfg(feature = "certification")]
        rx_app_cnt: 0,
    }
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
        if let PhyPayload::Data(DataPayload::Encrypted(data)) = uplink.get_payload() {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_key().into(), fcnt, &DefaultFactory));
            let uplink = data
                .decrypt(Some(&get_key().into()), Some(&get_key().into()), fcnt, &DefaultFactory)
                .unwrap();
            assert_eq!(uplink.fhdr().fcnt(), 0);
            let mut phy = DataPayloadCreator::new(rx_buffer).unwrap();
            let mut fctrl = FCtrl::new(0, false);
            fctrl.set_ack();
            phy.set_confirmed(false);
            phy.set_dev_addr([0; 4]);
            phy.set_uplink(false);
            phy.set_fctrl(&fctrl);
            // set ack bit
            let finished =
                phy.build(&[], [], &get_key().into(), &get_key().into(), &DefaultFactory).unwrap();
            finished.len()
        } else {
            panic!("Did not decode PhyPayload::Data!");
        }
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
