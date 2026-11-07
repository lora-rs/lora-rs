use lorawan::parser::{self, DecryptedDataPayload, PhyPayload};

use super::{Device, get_dev_addr, get_key, radio::*, region, timer::*};
use crate::mac::Session;
use crate::test_util::{Uplink, get_crypto};
use crate::{AppSKey, NwkSKey};
use std::sync::{Arc, atomic::AtomicUsize};

#[cfg(feature = "class-c")]
use super::net::{ExchangeAction, Net, Pkt};

fn default_session() -> Session {
    Session::new(NwkSKey::from(get_key()), AppSKey::from(get_key()), get_dev_addr())
}

pub fn session_with_region(region: region::Configuration) -> (RadioChannel, TimerChannel, Device) {
    let pending = Arc::new(AtomicUsize::new(0));
    let (radio_channel, mock_radio) = TestRadio::new(pending.clone());
    let (timer_channel, mock_timer) = TestTimer::new(pending);
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
    let pending = Arc::new(AtomicUsize::new(0));
    let (radio_channel, mock_radio) = TestRadio::new(pending.clone());
    let (timer_channel, mock_timer) = TestTimer::new(pending);
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

/// Parses the uplink, checks the MIC, and decrypts it in place, allowing
/// access to payload contents
pub fn decrypt_uplink(uplink: &mut Uplink) -> DecryptedDataPayload<'_> {
    let bytes = uplink.data_mut();
    let fcnt = match parser::parse(&*bytes) {
        Ok(PhyPayload::Data(data)) => {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_crypto(), fcnt));
            fcnt
        }
        _ => panic!("expected a data frame"),
    };
    DecryptedDataPayload::decrypt_in_place(bytes, Some(&get_crypto()), Some(&get_crypto()), fcnt)
        .unwrap()
}

/// The FCntUp of an uplink.
pub fn uplink_fcnt(uplink: &Uplink) -> u32 {
    decrypt_uplink(&mut uplink.clone()).fhdr().fcnt().into()
}

/// Check the uplink is a JoinRequest for the test credentials.
pub fn assert_join_request(uplink: &Uplink) {
    let mut uplink = uplink.clone();
    let Ok(PhyPayload::JoinRequest(join_request)) = parser::parse(uplink.data_mut()) else {
        panic!("expected a JoinRequest");
    };
    assert!(join_request.validate_mic(&get_crypto()), "JoinRequest MIC");
}

/// A class C device (US915) whose first uplink has been acknowledged, so
/// the RXC listen is active: the network has used FCntDown 0.
#[cfg(feature = "class-c")]
pub async fn setup_with_session_class_c() -> Net {
    let (radio, timer, mut device) = setup_with_session();
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.exchange(&[3, 2, 1], 3, false, ExchangeAction::rx1(Pkt::empty().ack())).await;
    r.expect_downlink_received();
    assert_eq!(r.fcnt_down(), Some(0));
    net
}

pub fn setup() -> (RadioChannel, TimerChannel, Device) {
    setup_internal(None)
}
