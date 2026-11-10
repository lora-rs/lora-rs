//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
use super::TestHandler;
use super::net;
use super::util;
use super::util::decrypt_uplink;
use crate::test_util::{get_crypto, get_dev_addr};
use lorawan::keys::Crypto;

use net::{ExchangeAction, Net, Pkt};
use std::sync::atomic::Ordering;

mod mac_common;

mod dlchannelreq_eu868;
mod join_cflist_eu868;
mod linkadrreq_eu868;
mod mac_in_app_payload_and_fopts;
mod mac_priority;
mod newchannelreq_eu868;
mod oversized_payload_eu868;
mod rxparamsetup_eu868;

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
    // Note: This test is region-agnostic
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    // Check that override is not set
    assert_eq!(device.mac.get_session().unwrap().override_confirmed, None);
    let mut net = Net::new(radio, timer, device).await;

    // TxFrameReqCtrlReq([2]) - DUT should switch to Confirmed
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("0702"))).await;
    r.expect_downlink_received();
    // Check that session is configured to override and send only confirmed packets
    assert_eq!(net.device().mac.get_session().unwrap().override_confirmed, Some(true));

    // TxFrameReqCtrlReq([0]) - no change
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("0700"))).await;
    r.expect_downlink_received();
    // Check that override_confirm has not changed!
    assert_eq!(net.device().mac.get_session().unwrap().override_confirmed, Some(true));
}

#[tokio::test]
async fn dut_join_req_resets_the_mac() {
    let (radio, timer, device) = util::setup_with_session();
    let mut net = Net::new(radio, timer, device).await;

    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("02"))).await;
    r.expect_rx_complete();

    assert!(net.device().get_session().is_none());
}

#[tokio::test]
async fn dut_reset_req_resets_the_device() {
    let (radio, timer, device) = util::setup_with_session_and_handler(TestHandler::default());
    let mut net = Net::new(radio, timer, device).await;

    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("01"))).await;
    r.expect_rx_complete();

    // Without a store there is nothing to resume, so the device restarts unjoined.
    assert!(net.device().get_session().is_none());
    assert_eq!(net.device().handler.reboots.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn tx_periodicity_change_overrides_the_uplink_period() {
    let (radio, timer, device) = util::setup_with_session();
    let mut net = Net::new(radio, timer, device).await;

    // TxPeriodicityChangeReq with index 1: one uplink every 5 seconds.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::cp_cmd("0601"))).await;
    r.expect_rx_complete();

    assert_eq!(net.device().override_periodicity, Some(5));
}
