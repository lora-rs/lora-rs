use super::*;
mod util;
use crate::test_util::*;
use util::*;

use crate::nb_device::Event;
use crate::radio::RfConfig;
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::parser::{self, DataFrameType, DecryptedDataPayload, PhyPayload};

/// Build an FPort 224 certification downlink with the given payload bytes.
#[cfg(feature = "certification")]
fn build_cert_downlink(buf: &mut [u8], payload_in_hex: &str, fcnt: u16) -> usize {
    let payload = hex::decode(payload_in_hex).unwrap();
    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: get_dev_addr(),
        ack: true,
        fcnt: fcnt.into(),
        payload: Payload::Data { f_port: NonZeroU8::new(224).unwrap(), data: &payload },
        ..Default::default()
    };
    let finished = frame.build_into(buf, &get_crypto(), Some(&get_crypto())).unwrap();
    finished.len()
}
#[test]
fn test_join_rx1() {
    let mut device = test_device();
    let response = device.join(get_otaa_credentials()).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(5000)));
    // send a timeout for beginning of window
    let response = device.handle_event(Event::TimeoutFired).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(5100)));
    device.get_radio().set_rxtx_handler(handle_join_request::<1>);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::JoinSuccess));
    assert!(device.get_session_keys().is_some());
}

#[test]
fn test_join_rx2() {
    let mut device = test_device();
    device.get_radio().set_rxtx_handler(handle_join_request::<2>);
    let response = device.join(get_otaa_credentials()).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(5000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(5100)));
    // send a timeout for end of rx2
    let response = device.handle_event(Event::TimeoutFired).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(6000)));
    // send a timeout for beginning of rx2
    let response = device.handle_event(Event::TimeoutFired).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(6100)));
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::JoinSuccess));
    assert!(device.get_session_keys().is_some());
}

#[test]
fn test_unconfirmed_uplink_no_downlink() {
    let mut device = test_device();
    device.join(get_abp_credentials()).unwrap();
    let response = device.send(&[0; 1], 1, false).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // being Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx2
    assert!(matches!(response, Response::RxComplete));
}
#[test]
fn test_unconfirmed_uplink_retransmission() {
    let mut device = test_device();
    device.join(get_abp_credentials()).unwrap();
    device.shared.mac.configuration.nb_trans = 2;
    let response = device.send(&[0; 1], 1, false).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let first = device.get_radio().take_last_uplink().unwrap();

    // RX windows of the first transmission time out
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    // End Rx2: NbTrans allows another transmission, so the radio is re-armed
    let response = device.handle_event(Event::TimeoutFired).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));

    // The retransmission resends the exact same frame (same FCntUp)
    let second = device.get_radio().take_last_uplink().unwrap();
    assert_eq!(first.data(), second.data());

    // RX windows of the retransmission time out; the FCntUp is consumed
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx2
    assert!(matches!(response, Response::RxComplete));
    assert_eq!(device.get_fcnt_up(), Some(1));
}

#[test]
fn test_unconfirmed_uplink_downlink_stops_retransmission() {
    let mut device = test_device();
    device.join(get_abp_credentials()).unwrap();
    device.shared.mac.configuration.nb_trans = 4;
    let response = device.send(&[0; 1], 1, false).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_req::<0, 0>);
    // A downlink in RX2 acknowledges the uplink: no retransmission
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(0)));
    assert_eq!(device.get_fcnt_up(), Some(1));
}

#[test]
fn test_confirmed_uplink_no_ack_retransmission() {
    let mut device = test_device();
    device.join(get_abp_credentials()).unwrap();
    device.shared.mac.configuration.nb_trans = 2;
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let _first = device.get_radio().take_last_uplink().unwrap();
    for attempt in 0..2 {
        let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
        assert!(matches!(response, Response::TimeoutRequest(1100)));
        let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
        assert!(matches!(response, Response::TimeoutRequest(2000)));
        let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx2
        assert!(matches!(response, Response::TimeoutRequest(2100)));
        let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx2
        if attempt == 0 {
            // NbTrans allows one more transmission
            assert!(matches!(response, Response::TimeoutRequest(1000)));
            let _second = device.get_radio().take_last_uplink().unwrap();
        } else {
            // No confirmation after all NbTrans attempts
            assert!(matches!(response, Response::NoAck));
        }
    }
    assert_eq!(device.get_fcnt_up(), Some(1));
}

#[test]
fn test_confirmed_uplink_no_ack() {
    let mut device = test_device();
    let response = device.join(get_abp_credentials());
    assert!(matches!(response, Ok(Response::JoinSuccess)));
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // being Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx2
    assert!(matches!(response, Response::NoAck));
}

#[test]
fn test_confirmed_uplink_with_ack_rx1() {
    let mut device = test_device();
    let response = device.join(get_abp_credentials());
    assert!(matches!(response, Ok(Response::JoinSuccess)));
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_req::<0, 0>);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(0)));
}

#[test]
fn test_confirmed_uplink_with_ack_rx2() {
    let mut device = test_device();
    let response = device.join(get_abp_credentials());
    assert!(matches!(response, Ok(Response::JoinSuccess)));
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // being Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_req::<0, 0>);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(0)));
}

#[cfg(feature = "certification")]
#[test]
fn test_certification_answer_uplink() {
    // An RxAppCntReq received in an RX window is answered with a regular
    // uplink: it opens its own RX windows.
    fn fp_rxappcnt_req(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_cert_downlink(buf, "09", 1) // RxAppCntReq
    }

    let mut device = test_device();
    device.join(get_abp_credentials()).unwrap();
    let response = device.send(&[0; 1], 1, false).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    device.get_radio().set_rxtx_handler(fp_rxappcnt_req);
    // The RxAppCntReq is answered in place; the answer opens its own RX1
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));

    // The answer is a data frame on FPort 224 carrying the RxAppCntAns;
    // the request downlink itself is counted, so the answer reports 1
    let mut answer = device.get_radio().take_last_uplink().unwrap();
    let uplink = match parser::parse(answer.data_mut()) {
        Ok(PhyPayload::Data(data)) => {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_crypto(), fcnt));
            DecryptedDataPayload::decrypt_in_place(
                answer.data_mut(),
                Some(&get_crypto()),
                Some(&get_crypto()),
                fcnt,
            )
            .unwrap()
        }
        _ => panic!("expected a data frame"),
    };
    // The user uplink was FCntUp 0; the accepted downlink incremented the
    // counter, so the answer is FCntUp 1
    assert_eq!(uplink.fhdr().fcnt(), 1);
    assert_eq!(uplink.f_port(), Some(224));
    assert_eq!(uplink.frm_payload(), lorawan::parser::FrmPayload::Data(&[0x09, 0x01, 0x00]));

    // The answer's RX windows time out; the uplink cycle completes
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin answer Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end answer Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin answer Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end answer Rx2
    assert!(matches!(response, Response::RxComplete));
    assert_eq!(device.get_fcnt_up(), Some(2));
}

#[cfg(feature = "certification")]
#[test]
fn test_certification_answer_retransmission() {
    // A certification answer is retransmitted per NbTrans when its own RX
    // windows time out without a downlink.
    fn fp_rxappcnt_req(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        build_cert_downlink(buf, "09", 1) // RxAppCntReq
    }

    let mut device = test_device();
    device.join(get_abp_credentials()).unwrap();
    device.shared.mac.configuration.nb_trans = 2;
    let response = device.send(&[0; 1], 1, false).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    device.get_radio().set_rxtx_handler(fp_rxappcnt_req);
    // The RxAppCntReq is answered in place; the answer opens its own RX1
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let first = device.get_radio().take_last_uplink().unwrap();

    // The answer's RX windows time out; NbTrans allows one more transmission
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin answer Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end answer Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin answer Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end answer Rx2
    assert!(matches!(response, Response::TimeoutRequest(1000)));

    // The retransmission resends the exact same answer frame (same FCntUp)
    let second = device.get_radio().take_last_uplink().unwrap();
    assert_eq!(first.data(), second.data());

    // No downlink after all NbTrans attempts: the uplink cycle completes
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin answer Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end answer Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin answer Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end answer Rx2
    assert!(matches!(response, Response::RxComplete));
    assert_eq!(device.get_fcnt_up(), Some(2));
}

#[test]
fn test_link_adr_ans() {
    let mut device = test_device();
    let response = device.join(get_abp_credentials());
    assert!(matches!(response, Ok(Response::JoinSuccess)));
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_req::<0, 0>);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(0)));
    // send another uplink which should carry the LinkAdrAns
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_ans);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::Phy(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(1)));
}
