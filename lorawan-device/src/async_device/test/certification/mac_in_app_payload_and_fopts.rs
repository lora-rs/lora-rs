//! LoRaWAN 1.0.4 Certification testcase 2.5.12.c
//! MAC Command(s) in App-Payload and/or Frame Options:
//! App-Payload and Frame Options.
//!
//! A downlink carrying MAC commands simultaneously in the FOpts field and in
//! the FPort = 0 FRMPayload must be silently discarded by the DUT: it must not
//! answer any of the commands, must not acknowledge a confirmed frame and must
//! not change its MAC state.
use super::{build_mac_in_fopts_and_frmpayload, decrypt_uplink, util};
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{Uplink, get_crypto};

use lorawan::parser::EncryptedDataPayload;

/// TCL payload from the certification test: LinkADRReq, LinkADRReq and
/// RxParamSetupReq in FOpts.
const FOPTS_MACS: &str = "030000006103000000610502d2ad84";
/// LinkADRReq, RxTimingSetupReq and DevStatusReq in the FPort = 0 FRMPayload.
const FRMPAYLOAD_MACS: &str = "0350000061080106";

/// Build the invalid frame and check that it is otherwise well formed, so that
/// the DUT is expected to reject it because of the both-fields rule and not
/// because of a bad MIC.
fn build_invalid(confirmed: bool, fcnt: u32, buf: &mut [u8]) -> usize {
    let len = build_mac_in_fopts_and_frmpayload(buf, FOPTS_MACS, FRMPAYLOAD_MACS, fcnt, confirmed);
    let frame = EncryptedDataPayload::parse(&buf[..len]).unwrap();
    assert!(frame.validate_mic(&get_crypto(), fcnt), "test frame must have a valid MIC");
    len
}

fn unconfirmed_both_fields(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
    build_invalid(false, 2, buf)
}

fn confirmed_both_fields(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
    build_invalid(true, 3, buf)
}

async fn assert_both_fields_frame_discarded(confirmed: bool) {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());

    let initial_data_rate = device.mac.configuration.data_rate;
    let initial_rx2_frequency = device.mac.configuration.rx2_frequency;

    // Step 1: DUT sends an uplink; TCL answers in RX2 with a frame carrying
    // MAC commands in both FOpts and FRMPayload. RX2 is used so the send
    // completes whether the frame is accepted or discarded.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 3, false).await;
        (device, response)
    });

    timer.fire_most_recent().await; // RX1 start
    radio.handle_timeout().await; // RX1 end
    timer.fire_most_recent().await; // RX2 start
    radio
        .handle_rxtx(if confirmed {
            confirmed_both_fields
        } else {
            unconfirmed_both_fields
        })
        .await;

    let (mut device, response) = task.await.unwrap();

    assert!(
        !matches!(response, Ok(SendResponse::DownlinkReceived(_))),
        "a frame with MAC commands in both FOpts and FRMPayload must be silently discarded, \
         got {response:?}"
    );
    assert!(
        matches!(response, Ok(SendResponse::RxComplete)),
        "expected RxComplete after the discarded frame, got {response:?}"
    );

    // No command may be answered ...
    let session = device.mac.get_session().unwrap();
    assert_eq!(
        session.uplink.mac_commands(),
        &[] as &[u8],
        "no MAC answer may be queued for a discarded frame"
    );
    // ... no ACK may be prepared, even for a confirmed downlink ...
    assert!(
        !session.uplink.confirms_downlink(),
        "a discarded confirmed downlink must not be acknowledged"
    );
    // ... and the MAC state must be untouched.
    assert_eq!(device.mac.configuration.data_rate, initial_data_rate);
    assert_eq!(device.mac.configuration.rx2_frequency, initial_rx2_frequency);

    // Step 2: the next uplink must not carry any MAC answers either.
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 4, false).await;
        (device, response)
    });

    timer.fire_most_recent().await; // RX1 start
    radio.handle_timeout().await; // RX1 end
    timer.fire_most_recent().await; // RX2 start
    radio.handle_timeout().await; // RX2 end

    let (_device, response) = task.await.unwrap();
    assert!(
        matches!(response, Ok(SendResponse::RxComplete)),
        "expected RxComplete, got {response:?}"
    );

    let mut uplink = radio.get_last_uplink().await;
    let dl = decrypt_uplink(&mut uplink);
    assert_eq!(
        dl.fhdr().f_opts(),
        &[] as &[u8],
        "the uplink must not piggyback answers to the discarded frame"
    );
    assert!(!dl.fhdr().fctrl().ack(), "the uplink must not acknowledge the discarded frame");
}

#[tokio::test]
async fn eu868_unconfirmed_mac_in_app_payload_and_fopts_discarded() {
    assert_both_fields_frame_discarded(false).await;
}

#[tokio::test]
async fn eu868_confirmed_mac_in_app_payload_and_fopts_discarded() {
    assert_both_fields_frame_discarded(true).await;
}
