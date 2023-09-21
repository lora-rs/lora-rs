use super::*;
mod util;
use crate::test_util::*;
use util::*;

use crate::Event;
#[test]
fn test_join_rx1() {
    let mut device = test_device(get_otaa_credentials());
    let response = device.handle_event(Event::NewSessionRequest).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(5000)));
    // send a timeout for beginning of window
    let response = device.handle_event(Event::TimeoutFired).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(5100)));
    device.get_radio().set_rxtx_handler(handle_join_request);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::PhyEvent(()))).unwrap();
    assert!(matches!(response, Response::JoinSuccess));
    assert!(device.get_session_keys().is_some());
}

#[test]
fn test_join_rx2() {
    let mut device = test_device(get_otaa_credentials());
    device.get_radio().set_rxtx_handler(handle_join_request);
    let response = device.handle_event(Event::NewSessionRequest).unwrap();
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
    let response = device.handle_event(Event::RadioEvent(radio::Event::PhyEvent(()))).unwrap();
    assert!(matches!(response, Response::JoinSuccess));
    assert!(device.get_session_keys().is_some());
}

#[test]
fn test_unconfirmed_uplink_no_downlink() {
    let mut device = test_device(get_abp_credentials());
    let response = device.send(&[0; 1], 1, false).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // being Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx2
    assert!(matches!(response, Response::ReadyToSend));
}
#[test]
fn test_confirmed_uplink_no_ack() {
    let mut device = test_device(get_abp_credentials());
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
    let mut device = test_device(get_abp_credentials());
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_req);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::PhyEvent(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(0)));
}

#[test]
fn test_confirmed_uplink_with_ack_rx2() {
    let mut device = test_device(get_abp_credentials());
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // end Rx1
    assert!(matches!(response, Response::TimeoutRequest(2000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // being Rx2
    assert!(matches!(response, Response::TimeoutRequest(2100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_req);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::PhyEvent(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(0)));
}

#[test]
fn test_link_adr_ans() {
    let mut device = test_device(get_abp_credentials());
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_req);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::PhyEvent(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(0)));
    // send another uplink which should carry the LinkAdrAns
    let response = device.send(&[0; 1], 1, true).unwrap();
    assert!(matches!(response, Response::TimeoutRequest(1000)));
    let response = device.handle_event(Event::TimeoutFired).unwrap(); // begin Rx1
    assert!(matches!(response, Response::TimeoutRequest(1100)));
    device.get_radio().set_rxtx_handler(handle_data_uplink_with_link_adr_ans);
    // send a radio event to let the radio device indicate a packet was received
    let response = device.handle_event(Event::RadioEvent(radio::Event::PhyEvent(()))).unwrap();
    assert!(matches!(response, Response::DownlinkReceived(1)));
}
