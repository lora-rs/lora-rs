//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{get_crypto, get_dev_addr, Uplink};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::parser::{self, DataFrameType, DecryptedDataPayload, PhyPayload};

use std::sync::Arc;
use tokio::sync::Mutex;

mod mac_common;

mod dlchannelreq_eu868;
mod mac_priority;
mod newchannelreq_eu868;
mod oversized_payload_eu868;
mod rxparamsetup_eu868;

/// Parses the uplink, checks the MIC, and decrypts it in place, allowing
/// access to payload contents
fn decrypt_uplink(uplink: &mut Uplink) -> DecryptedDataPayload<'_> {
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

fn _build(buf: &mut [u8], payload_in_hex: &str, fcnt: u16, fport: u8) -> usize {
    let payload = hex::decode(payload_in_hex).unwrap();
    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: get_dev_addr(),
        ack: true,
        fcnt: fcnt.into(),
        payload: match fport {
            0 => Payload::MacCommands(&payload),
            p => Payload::Data { f_port: NonZeroU8::new(p).unwrap(), data: &payload },
        },
        ..Default::default()
    };
    let finished = frame.build_into(buf, &get_crypto(), Some(&get_crypto())).unwrap();
    finished.len()
}

/// Build a packet on the given fport with MAC commands piggybacked in FOpts
fn packet_with_mac(
    buf: &mut [u8],
    fport: u8,
    payload_in_hex: &str,
    mac_in_hex: &str,
    fcnt: u16,
) -> usize {
    let payload = hex::decode(payload_in_hex).unwrap();
    let cmds = hex::decode(mac_in_hex).unwrap();
    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: get_dev_addr(),
        ack: true,
        fcnt: fcnt.into(),
        f_opts: &cmds,
        payload: Payload::Data { f_port: NonZeroU8::new(fport).unwrap(), data: &payload },
        ..Default::default()
    };
    let finished = frame.build_into(buf, &get_crypto(), Some(&get_crypto())).unwrap();
    finished.len()
}

/// Build fport = 0 packet with MAC commands in fopts
fn build_mac(buf: &mut [u8], payload_in_hex: &str, fcnt: u16) -> usize {
    _build(buf, payload_in_hex, fcnt, 0)
}

/// Build certification protocol packet (fport = 224)
fn build_packet(buf: &mut [u8], payload_in_hex: &str, fcnt: u16) -> usize {
    _build(buf, payload_in_hex, fcnt, 224)
}

#[tokio::test]
async fn txframectrlreq_no_change() {
    // This test will check how TxFrameReqCtrlReq is handled and
    // whether it overrides confirmation flag for packets properly
    fn txframectrlreq_override_confirmed(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        // TxFrameReqCtrlReq([2]) - DUT should switch to Confirmed
        build_packet(buf, "0702", 1)
    }

    fn txframectrlreq_no_change(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        buf: &mut [u8],
    ) -> usize {
        // TxFrameReqCtrlReq([0]) - no change
        build_packet(buf, "0700", 2)
    }

    // Note: This test is region-agnostic
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    // Check that override is not set
    if let Some(session) = device.mac.get_session() {
        assert_eq!(session.override_confirmed, None);
    }

    // Trigger uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    radio.handle_rxtx(txframectrlreq_override_confirmed).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }
    // Check that session is configured to override and send only confirmed packets
    if let Some(session) = device.mac.get_session() {
        assert_eq!(session.override_confirmed, Some(true));
    }

    // Trigger second uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    timer.fire_most_recent().await;
    // TxFrameReqCtrl - no_change is no-op
    radio.handle_rxtx(txframectrlreq_no_change).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }
    // Check that override_confirm has not changed!
    if let Some(session) = device.mac.get_session() {
        assert_eq!(session.override_confirmed, Some(true));
    }
}
