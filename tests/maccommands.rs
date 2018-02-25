// Copyright (c) 2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

extern crate lorawan;

use lorawan::maccommands::*;

macro_rules! test_helper {
    ( $data:ident, $name:ident, $type:ident, $size:expr, $( ( $method:ident, $val:expr ) ,)*) => {{
        {
            assert!($type::new(&$data[0..0]).is_err());
            let mc = $type::new(&$data[..]);
            assert!(mc.is_ok());
            if let (MacCommand::$name(res), size) = mc.unwrap() {
                assert_eq!(size, $size);
                $(
                    assert_eq!(res.$method(), $val);
                )*
            } else {
                panic!("failed to parse {}", stringify!($type));
            }
        }
    }};

    ( $name:ident, $type:ident ) => {{
        {
            let data = vec![];
            let mc = $type::new(&data[..]);
            assert!(mc.is_ok());
            if let (MacCommand::$name(_), size) = mc.unwrap() {
                assert_eq!(size, 0);
            } else {
                panic!("failed to parse {}", stringify!($type));
            }
        }
    }};
}

#[test]
fn test_link_check_req_new() {
    test_helper!(LinkCheckReq, LinkCheckReqPayload);
}

#[test]
fn test_link_check_ans_new() {
    let data = vec![0xa, 0x0f];
    test_helper!(
        data,
        LinkCheckAns,
        LinkCheckAnsPayload,
        2,
        (margin, 10),
        (gateway_count, 15),
    );
}

#[test]
fn test_link_adr_req_new() {
    let data = vec![0x12, 0x04, 0x00, 0x45];
    let expected_channel_mask = ChannelMask::new(&[0x04, 0x00]).unwrap();
    test_helper!(
        data,
        LinkADRReq,
        LinkADRReqPayload,
        4,
        (data_rate, 1),
        (tx_power, 2),
        (channel_mask, expected_channel_mask),
        (redundancy, Redundancy::new(0x45)),
    );
}

#[test]
fn test_link_adr_ans_new() {
    let examples = [
        ([0x00], false, false, false, false),
        ([0x01], true, false, false, false),
        ([0x02], false, true, false, false),
        ([0x04], false, false, true, false),
        ([0x07], true, true, true, true),
    ];
    assert!(LinkADRReqPayload::new(&examples[0].0[0..0]).is_err());
    for &(ref v, ref e_power, ref e_dr, ref e_cm, ref e_ack) in &examples {
        let mc = LinkADRAnsPayload::new(&v[..]);
        assert!(mc.is_ok());
        if let (MacCommand::LinkADRAns(laa), size) = mc.unwrap() {
            assert_eq!(size, 1);
            assert_eq!(laa.channel_mask_ack(), *e_power);
            assert_eq!(laa.data_rate_ack(), *e_dr);
            assert_eq!(laa.powert_ack(), *e_cm);
            assert_eq!(laa.ack(), *e_ack);
        } else {
            panic!("failed to parse LinkADRAnsPayload");
        }
    }
}

#[test]
fn test_duty_cycle_req_new() {
    let data = vec![0x02];
    test_helper!(
        data,
        DutyCycleReq,
        DutyCycleReqPayload,
        1,
        (max_duty_cycle_raw, 2),
        (max_duty_cycle, 0.25),
    );
}

#[test]
fn test_duty_cycle_ans_new() {
    test_helper!(DutyCycleAns, DutyCycleAnsPayload);
}

#[test]
fn test_rx_param_setup_req_new() {
    let data = vec![0x3b, 0x01, 0x02, 0x04];
    test_helper!(
        data,
        RXParamSetupReq,
        RXParamSetupReqPayload,
        4,
        (dl_settings, DLSettings::new(0x3b)),
        (frequency, Frequency::new_from_raw(&data[1..])),
    );
}

#[test]
fn test_rx_param_setup_ans_new() {
    let examples = [
        ([0x00], false, false, false, false),
        ([0x01], true, false, false, false),
        ([0x02], false, true, false, false),
        ([0x04], false, false, true, false),
        ([0x07], true, true, true, true),
    ];
    assert!(RXParamSetupAnsPayload::new(&examples[0].0[0..0]).is_err());
    for &(ref v, ref e_ch, ref e_rx2_dr, ref e_rx1_dr_offset, ref e_ack) in &examples {
        let mc = RXParamSetupAnsPayload::new(&v[..]);
        assert!(mc.is_ok());
        if let (MacCommand::RXParamSetupAns(psa), size) = mc.unwrap() {
            assert_eq!(size, 1);
            assert_eq!(psa.channel_ack(), *e_ch);
            assert_eq!(psa.rx2_data_rate_ack(), *e_rx2_dr);
            assert_eq!(psa.rx1_dr_offset_ack(), *e_rx1_dr_offset);
            assert_eq!(psa.ack(), *e_ack);
        } else {
            panic!("failed to parse RXParamSetupAnsPayload");
        }
    }
}

#[test]
fn test_dev_status_req() {
    test_helper!(DevStatusReq, DevStatusReqPayload);
}

#[test]
fn test_dev_status_ans() {
    let data = vec![0xfe, 0x3f];
    test_helper!(
        data,
        DevStatusAns,
        DevStatusAnsPayload,
        2,
        (battery, 254),
        (margin, -1),
    );
}

#[test]
fn test_new_channel_req() {
    let data = vec![0x03, 0x01, 0x02, 0x04, 0x5a];
    test_helper!(
        data,
        NewChannelReq,
        NewChannelReqPayload,
        5,
        (channel_index, 3),
        (frequency, Frequency::new_from_raw(&data[1..4])),
        (data_rate_range, DataRateRange::new(data[4])),
    );
}

#[test]
fn test_new_channel_ans() {
    let examples = [
        ([0x00], false, false, false),
        ([0x01], true, false, false),
        ([0x02], false, true, false),
        ([0x03], true, true, true),
    ];
    assert!(NewChannelAnsPayload::new(&examples[0].0[0..0]).is_err());
    for &(ref v, ref e_ch_freq, ref e_drr, ref e_ack) in &examples {
        let mc = NewChannelAnsPayload::new(&v[..]);
        assert!(mc.is_ok());
        if let (MacCommand::NewChannelAns(nca), size) = mc.unwrap() {
            assert_eq!(size, 1);
            assert_eq!(nca.data_rate_range_ack(), *e_drr);
            assert_eq!(nca.channel_freq_ack(), *e_ch_freq);
            assert_eq!(nca.ack(), *e_ack);
        } else {
            panic!("failed to parse RXParamSetupAnsPayload");
        }
    }
}

#[test]
fn test_rx_timing_setup_req() {
    let data = vec![0x02];
    test_helper!(
        data,
        RXTimingSetupReq,
        RXTimingSetupReqPayload,
        1,
        (delay, 2),
    );
}

#[test]
fn test_rx_timing_setup_ans() {
    test_helper!(RXTimingSetupAns, RXTimingSetupAnsPayload);
}

#[test]
fn test_parse_mac_commands_empty_downlink() {
    let data = mac_cmds_payload();
    assert!(parse_mac_commands(&data[0..0], false).is_ok());
    assert_eq!(parse_mac_commands(&data[0..0], false).unwrap().len(), 0);
}

#[test]
fn test_parse_mac_commands_empty_uplink() {
    let data = mac_cmds_payload();
    assert!(parse_mac_commands(&data[0..0], true).is_ok());
    assert_eq!(parse_mac_commands(&data[0..0], true).unwrap().len(), 0);
}

#[test]
fn test_parse_mac_commands_with_multiple_cmds() {
    let data = mac_cmds_payload();
    let mcs = parse_mac_commands(&data[..], true);
    assert!(mcs.is_ok());
    let commands = mcs.unwrap();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0], MacCommand::LinkCheckReq(LinkCheckReqPayload()));
    let expected = LinkADRAnsPayload::new(&data[2..]).unwrap().0;
    assert_eq!(commands[1], expected);
}

fn mac_cmds_payload() -> Vec<u8> {
    vec![LinkCheckReqPayload::cid(), LinkADRAnsPayload::cid(), 0x00]
}

#[test]
fn test_dl_settings() {
    let dl_settings = DLSettings::new(0x5b);
    assert_eq!(dl_settings.rx1_dr_offset(), 0x05);
    assert_eq!(dl_settings.rx2_data_rate(), 0x0b);
}

#[test]
fn test_channel_mask() {
    let data = vec![0x03, 0x10];
    let mut expected = vec![false; 16];
    expected[0] = true;
    expected[1] = true;
    expected[12] = true;
    let chan_mask = ChannelMask::new(&data[..]);
    assert!(chan_mask.is_ok());
    assert_eq!(chan_mask.unwrap().statuses(), expected);
}

#[test]
fn test_redundancy_channel_mask_control() {
    let redundancy = Redundancy::new(0x7f);
    assert_eq!(redundancy.channel_mask_control(), 0x07);
}

#[test]
fn test_redundancy_number_of_transmissions() {
    let redundancy = Redundancy::new(0x7f);
    assert_eq!(redundancy.number_of_transmissions(), 0x0f);
}

#[test]
fn test_frequency_new_bad_payload() {
    let data = frequency_payload();
    assert!(Frequency::new(&data[0..0]).is_none());
}

#[test]
fn test_frequency_value() {
    let data = frequency_payload();
    let freq = Frequency::new(&data[..]);
    assert!(freq.is_some());
    assert_eq!(freq.unwrap().value(), 26265700);
}

fn frequency_payload() -> Vec<u8> {
    vec![0x01, 0x02, 0x04]
}

#[test]
fn test_data_rate_range() {
    let drr = DataRateRange::new(0x5a);
    assert_eq!(drr.max_data_rate(), 0x05);
    assert_eq!(drr.min_data_range(), 0x0a);
}
