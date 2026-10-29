//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{Uplink, get_crypto, get_dev_addr};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::keys::Crypto;
use lorawan::parser::{self, DataFrameType, DecryptedDataPayload, PhyPayload};

use std::sync::Arc;
use tokio::sync::Mutex;

mod mac_common;

mod dlchannelreq_eu868;
mod linkadrreq_eu868;
mod mac_in_app_payload_and_fopts;
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

/// Build an invalid downlink which carries MAC commands both in the FOpts
/// field, and in the FRMPayload of a FPort = 0 frame.
fn build_mac_in_fopts_and_frmpayload(
    buf: &mut [u8],
    fopts_in_hex: &str,
    payload_in_hex: &str,
    fcnt: u32,
    confirmed: bool,
) -> usize {
    let fopts = hex::decode(fopts_in_hex).unwrap();
    let macs = hex::decode(payload_in_hex).unwrap();
    assert!(fopts.len() <= 15);
    let crypto = get_crypto();

    let fhdr_len = 7 + fopts.len();
    let fport_at = 1 + fhdr_len;
    let payload_at = fport_at + 1;
    let payload_end = payload_at + macs.len();
    let total = payload_end + 4;
    let out = &mut buf[..total];

    out[0] = if confirmed {
        0xa0
    } else {
        0x60
    }; // (Un)ConfirmedDataDown
    out[1..5].copy_from_slice(get_dev_addr().as_wire_bytes());
    out[5] = fopts.len() as u8;
    out[6..8].copy_from_slice(&(fcnt as u16).to_le_bytes());
    out[8..8 + fopts.len()].copy_from_slice(&fopts);
    out[fport_at] = 0;
    out[payload_at..payload_end].copy_from_slice(&macs);

    // Encrypt the FPort 0 FRMPayload with the NwkSKey
    let mut a = [0u8; 16];
    a[0] = 0x01;
    a[5] = (out[0] & 0x20) >> 5;
    a[6..10].copy_from_slice(&out[1..5]);
    a[10..14].copy_from_slice(&fcnt.to_le_bytes());
    for (i, block) in out[payload_at..payload_end].chunks_mut(16).enumerate() {
        a[15] = (i + 1) as u8;
        let mut s = a;
        crypto.encrypt_block(&mut s);
        for (byte, keystream) in block.iter_mut().zip(s) {
            *byte ^= keystream;
        }
    }

    // MIC over MHDR | FHDR | FPort | FRMPayload
    let mut b0 = [0u8; 16];
    b0[0] = 0x49;
    b0[5] = (out[0] & 0x20) >> 5;
    b0[6..10].copy_from_slice(&out[1..5]);
    b0[10..14].copy_from_slice(&fcnt.to_le_bytes());
    b0[15] = payload_end as u8;
    let mic = crypto.calculate_mic(&b0, &out[..payload_end]);
    out[payload_end..total].copy_from_slice(&mic);

    total
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
