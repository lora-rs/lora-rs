// Copyright (c) 2018,2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use lorawan::maccommandcreator::*;
use lorawan::maccommands::*;

#[test]
fn test_link_check_req_creator() {
    let creator = LinkCheckReqCreator::new();
    let res = creator.build();
    assert_eq!(res, [0x02]);
}

#[test]
fn test_link_check_ans_creator() {
    let mut creator = LinkCheckAnsCreator::new();
    let res = creator.set_margin(253).set_gateway_count(254).build();
    assert_eq!(res, [0x02, 0xfd, 0xfe]);
}

#[test]
fn test_link_adr_req_creator() {
    let mut creator = LinkADRReqCreator::new();
    let channel_mask_bytes = [0xc7, 0x0b];
    let res = creator
        .set_data_rate(0x05)
        .unwrap()
        .set_tx_power(0x03)
        .unwrap()
        .set_channel_mask(channel_mask_bytes)
        .set_redundancy(0x37)
        .build();
    assert_eq!(res, [0x03, 0x53, 0xc7, 0x0b, 0x37]);
}

#[test]
fn test_link_adr_req_creator_bad_data_rate() {
    let mut creator = LinkADRReqCreator::new();
    assert!(creator.set_data_rate(0x10).is_err());
}

#[test]
fn test_link_adr_req_creator_bad_tx_power() {
    let mut creator = LinkADRReqCreator::new();
    assert!(creator.set_tx_power(0x10).is_err());
}

#[test]
fn test_link_adr_ans_creator() {
    let mut creator = LinkADRAnsCreator::new();
    let res =
        creator.set_channel_mask_ack(true).set_data_rate_ack(true).set_tx_power_ack(true).build();
    assert_eq!(res, [0x03, 0x07]);
}

#[test]
fn test_duty_cycle_req_creator() {
    let mut creator = DutyCycleReqCreator::new();
    let res = creator.set_max_duty_cycle(0x0f).unwrap().build();
    assert_eq!(res, [DutyCycleReqPayload::cid(), 0x0f]);
}

#[test]
fn test_duty_cycle_ans_creator() {
    let creator = DutyCycleAnsCreator::new();
    let res = creator.build();
    assert_eq!(res, [DutyCycleAnsPayload::cid()]);
}

#[test]
fn test_rx_param_setup_req_creator() {
    let mut creator = RXParamSetupReqCreator::new();
    let res = creator.set_dl_settings(0xcd).set_frequency(&[0x12, 0x34, 0x56]).build();
    assert_eq!(res, [RXParamSetupReqPayload::cid(), 0xcd, 0x12, 0x34, 0x56]);
}

#[test]
fn test_rx_param_setup_ans_creator() {
    let mut creator = RXParamSetupAnsCreator::new();
    let res = creator
        .set_channel_ack(true)
        .set_rx2_data_rate_ack(true)
        .set_rx1_data_rate_offset_ack(true)
        .build();
    assert_eq!(res, [RXParamSetupAnsPayload::cid(), 0x07]);
}

#[test]
fn test_dev_status_req_creator() {
    let creator = DevStatusReqCreator::new();
    let res = creator.build();
    assert_eq!(res, [DevStatusReqPayload::cid()]);
}

#[test]
fn test_dev_status_ans_creator() {
    let mut creator = DevStatusAnsCreator::new();
    let res = creator.set_battery(0xfe).set_margin(-32).unwrap().build();
    assert_eq!(res, [DevStatusAnsPayload::cid(), 0xfe, 0x20]);
}

#[test]
fn test_dev_status_ans_creator_too_big_margin() {
    let mut creator = DevStatusAnsCreator::new();
    assert!(creator.set_margin(32).is_err());
}

#[test]
fn test_dev_status_ans_creator_too_small_margin() {
    let mut creator = DevStatusAnsCreator::new();
    assert!(creator.set_margin(-33).is_err());
}

#[test]
fn test_new_channel_req_creator() {
    let mut creator = NewChannelReqCreator::new();
    let res = creator
        .set_channel_index(0x0f)
        .set_frequency(&[0x12, 0x34, 0x56])
        .set_data_rate_range(0x53)
        .build();
    assert_eq!(res, [NewChannelReqPayload::cid(), 0x0f, 0x12, 0x34, 0x56, 0x53]);
}

#[test]
fn test_new_channel_ans_creator() {
    let mut creator = NewChannelAnsCreator::new();
    let res = creator.set_channel_frequency_ack(true).set_data_rate_range_ack(true).build();
    assert_eq!(res, [NewChannelAnsPayload::cid(), 0x03]);
}

#[test]
fn test_rx_timing_setup_req_creator() {
    let mut creator = RXTimingSetupReqCreator::new();
    let res = creator.set_delay(0x0f).unwrap().build();
    assert_eq!(res, [RXTimingSetupReqPayload::cid(), 0x0f]);
}

#[test]
fn test_rx_timing_setup_ans_creator() {
    let creator = RXTimingSetupAnsCreator::new();
    assert_eq!(creator.build(), [RXTimingSetupAnsPayload::cid()]);
}

#[test]
fn test_rx_timing_setup_req_creator_bad_delay() {
    let mut creator = RXTimingSetupReqCreator::new();
    assert!(creator.set_delay(0x10).is_err());
}

#[test]
fn test_tx_param_setup_req_creator() {
    let mut creator = TXParamSetupReqCreator::new();
    creator.set_downlink_dwell_time();
    creator.set_uplink_dwell_time().set_uplink_dwell_time();
    creator.set_max_eirp(3).unwrap();
    let res = creator.build();
    assert_eq!(res, [TXParamSetupReqPayload::cid(), 0b110011]);
}

#[test]
fn test_tx_param_setup_req_creator_bad_max_eirp() {
    let mut creator = TXParamSetupReqCreator::new();
    assert!(creator.set_max_eirp(17).is_err());
}

#[test]
fn test_dl_channel_req_creator() {
    let mut creator = DlChannelReqCreator::new();
    creator.set_channel_index(3);
    creator.set_frequency(&[0x12, 0x34, 0x56]);
    let res = creator.build();
    assert_eq!(res, [DlChannelReqPayload::cid(), 0x03, 0x12, 0x34, 0x56]);
}

#[test]
fn test_device_time_req_creator() {
    let creator = DeviceTimeReqCreator::new();
    let res = creator.build();
    assert_eq!(res, [DeviceTimeReqPayload::cid()]);
}

#[test]
fn test_device_time_ans_creator() {
    let mut creator = DeviceTimeAnsCreator::new();
    creator.set_seconds(123456);
    creator.set_nano_seconds(123456789).unwrap();
    let res = creator.build();
    assert_eq!(res, [DeviceTimeAnsPayload::cid(), 64, 226, 1, 0, 31]);
}

#[test]
fn test_build_mac_commands() {
    let rx_timing_setup_req = RXTimingSetupReqPayload::new_as_mac_cmd(&[0x02]).unwrap().0;
    let dev_status_ans = DevStatusAnsPayload::new_as_mac_cmd(&[0xfe, 0x3f]).unwrap().0;
    let cmds: Vec<&dyn SerializableMacCommand> = vec![&rx_timing_setup_req, &dev_status_ans];
    let expected_len = mac_commands_len(&cmds[..]);
    let mut res = vec![0; expected_len];
    assert_eq!(build_mac_commands(&cmds[..], &mut res[..]), Ok(expected_len));
    assert_eq!(res, &vec![0x08, 0x02, 0x06, 0xfe, 0x3f][..]);
}
