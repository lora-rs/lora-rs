// Copyright (c) 2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

extern crate lorawan;

use lorawan::maccommands::*;

#[test]
fn test_link_check_req_new() {
    let data = vec![];
    let mc = LinkCheckReqPayload::new(&data[0..0]);
    assert!(mc.is_ok());
    if let (MacCommand::LinkCheckReq(_), size) = mc.unwrap() {
        assert_eq!(size, 0);
    }
}

#[test]
fn test_link_check_ans_new() {
    let data = vec![0xa, 0x0f];
    assert!(LinkCheckAnsPayload::new(&data[0..0]).is_err());
    let mc = LinkCheckAnsPayload::new(&data[..]);
    assert!(mc.is_ok());
    if let (MacCommand::LinkCheckAns(lcr), size) = mc.unwrap() {
        assert_eq!(size, 2);
        assert_eq!(lcr.margin(), 10);
        assert_eq!(lcr.gateway_count(), 15);
    } else {
        panic!("failed to parse LinkADRAnsPayload");
    }
}

#[test]
fn test_link_adr_req_new() {
    let data = vec![0x12, 0x04, 0x00, 0x45];
    assert!(LinkADRReqPayload::new(&data[0..0]).is_err());
    let mc = LinkADRReqPayload::new(&data[..]);
    assert!(mc.is_ok());
    if let (MacCommand::LinkADRReq(lar), size) = mc.unwrap() {
        assert_eq!(size, 4);
        assert_eq!(lar.data_rate(), 1);
        assert_eq!(lar.tx_power(), 2);
        let expected_channel_mask = ChannelMask::new(&[0x04, 0x00]).unwrap();
        assert_eq!(lar.channel_mask(), expected_channel_mask);
        assert_eq!(lar.redundancy(), Redundancy::new(0x45));
    } else {
        panic!("failed to parse LinkADRAnsPayload");
    }
}

#[test]
fn test_link_adr_ans_new() {
    let examples = [
        ([0x00], false, false, false),
        ([0x01], true, false, false),
        ([0x02], false, true, false),
        ([0x04], false, false, true),
    ];
    assert!(LinkADRReqPayload::new(&examples[0].0[0..0]).is_err());
    for &(ref v, ref e_power, ref e_dr, ref e_cm) in &examples {
        let mc = LinkADRAnsPayload::new(&v[..]);
        assert!(mc.is_ok());
        if let (MacCommand::LinkADRAns(laa), size) = mc.unwrap() {
            assert_eq!(size, 1);
            assert_eq!(laa.channel_mask_ack(), *e_power);
            assert_eq!(laa.data_rate_ack(), *e_dr);
            assert_eq!(laa.powert_ack(), *e_cm);
        } else {
            panic!("failed to parse LinkADRAnsPayload");
        }
    }
}

#[test]
fn test_duty_cycle_req_new() {
    let data = vec![0x02];
    assert!(DutyCycleReqPayload::new(&data[0..0]).is_err());
    let mc = DutyCycleReqPayload::new(&data[..]);
    assert!(mc.is_ok());
    if let (MacCommand::DutyCycleReq(dcr), size) = mc.unwrap() {
        assert_eq!(size, 1);
        assert_eq!(dcr.max_duty_cycle_raw(), 2);
        assert_eq!(dcr.max_duty_cycle(), 0.25);
    } else {
        panic!("failed to parse DutyCycleReqPayload");
    }
}

#[test]
fn test_duty_cycle_ans_new() {
    let data = vec![];
    let mc = DutyCycleAnsPayload::new(&data[0..0]);
    assert!(mc.is_ok());
    if let (MacCommand::DutyCycleAns(_), size) = mc.unwrap() {
        assert_eq!(size, 0);
    }
}

#[test]
fn test_rx_param_setup_req_new() {
    let data = vec![0x3b, 0x01, 0x02, 0x04];
    assert!(RXParamSetupReqPayload::new(&data[0..0]).is_err());
    let mc = RXParamSetupReqPayload::new(&data[..]);
    assert!(mc.is_ok());
    if let (MacCommand::RXParamSetupReq(psr), size) = mc.unwrap() {
        assert_eq!(size, 4);
        assert_eq!(psr.dl_settings(), DLSettings::new(0x3b));
        assert_eq!(psr.frequency(), Frequency::new_from_raw(&data[1..]));
    } else {
        panic!("failed to parse RXParamSetupReqPayload");
    }
}

#[test]
fn test_rx_param_setup_ans_new() {
    let examples = [
        ([0x00], false, false, false),
        ([0x01], true, false, false),
        ([0x02], false, true, false),
        ([0x04], false, false, true),
    ];
    assert!(RXParamSetupAnsPayload::new(&examples[0].0[0..0]).is_err());
    for &(ref v, ref e_ch, ref e_rx2_dr, ref e_rx1_dr_offset) in &examples {
        let mc = RXParamSetupAnsPayload::new(&v[..]);
        assert!(mc.is_ok());
        if let (MacCommand::RXParamSetupAns(psa), size) = mc.unwrap() {
            assert_eq!(size, 1);
            assert_eq!(psa.channel_ack(), *e_ch);
            assert_eq!(psa.rx2_data_rate_ack(), *e_rx2_dr);
            assert_eq!(psa.rx1_dr_offset_ack(), *e_rx1_dr_offset);
        } else {
            panic!("failed to parse RXParamSetupAnsPayload");
        }
    }
}

#[test]
fn test_dev_status_req() {
    let data = vec![];
    let mc = DevStatusReqPayload::new(&data[0..0]);
    assert!(mc.is_ok());
    if let (MacCommand::DevStatusReq(_), size) = mc.unwrap() {
        assert_eq!(size, 0);
    }
}

#[test]
fn test_dev_status_ans() {
    let data = vec![0xfe, 0x3f];
    assert!(DevStatusAnsPayload::new(&data[0..0]).is_err());
    let mc = DevStatusAnsPayload::new(&data[..]);
    assert!(mc.is_ok());
    if let (MacCommand::DevStatusAns(dsa), size) = mc.unwrap() {
        assert_eq!(size, 2);
        assert_eq!(dsa.battery(), 254);
        assert_eq!(dsa.margin(), -1);
    } else {
        panic!("failed to parse DevStatusAnsPayload");
    }
}

#[test]
fn test_new_channel_req() {
    let data = vec![0x03, 0x01, 0x02, 0x04, 0x5a];
    assert!(NewChannelReqPayload::new(&data[0..0]).is_err());
    let mc = NewChannelReqPayload::new(&data[..]);
    assert!(mc.is_ok());
    if let (MacCommand::NewChannelReq(ncr), size) = mc.unwrap() {
        assert_eq!(size, 5);
        assert_eq!(ncr.channel_index(), 3);
        assert_eq!(ncr.frequency(), Frequency::new_from_raw(&data[1..4]));
        assert_eq!(ncr.data_rate_range(), DataRateRange::new(data[4]));
    } else {
        panic!("failed to parse RXParamSetupReqPayload");
    }
}

#[test]
fn test_new_channel_ans() {
    let examples = [
        ([0x00], false, false),
        ([0x01], true, false),
        ([0x02], false, true),
    ];
    assert!(NewChannelAnsPayload::new(&examples[0].0[0..0]).is_err());
    for &(ref v, ref ch_freq, ref drr) in &examples {
        let mc = NewChannelAnsPayload::new(&v[..]);
        assert!(mc.is_ok());
        if let (MacCommand::NewChannelAns(nca), size) = mc.unwrap() {
            assert_eq!(size, 1);
            assert_eq!(nca.data_rate_range_ack(), *drr);
            assert_eq!(nca.channel_freq_ack(), *ch_freq);
        } else {
            panic!("failed to parse RXParamSetupAnsPayload");
        }
    }
}

#[test]
fn test_rx_timing_setup_req() {
    let data = vec![0x02];
    assert!(RXTimingSetupReqPayload::new(&data[0..0]).is_err());
    let mc = RXTimingSetupReqPayload::new(&data[..]);
    assert!(mc.is_ok());
    if let (MacCommand::RXTimingSetupReq(tsr), size) = mc.unwrap() {
        assert_eq!(size, 1);
        assert_eq!(tsr.delay(), 2);
    } else {
        panic!("failed to parse RXTimingSetupReqPayload");
    }
}

#[test]
fn test_rx_timing_setup_ans() {
    let data = vec![];
    let mc = RXTimingSetupAnsPayload::new(&data[0..0]);
    assert!(mc.is_ok());
    if let (MacCommand::RXTimingSetupAns(_), size) = mc.unwrap() {
        assert_eq!(size, 0);
    }
}

// TODO: Test parse_mac_commands, DLSettings, ChannelMask, Redundancy, Frequency, DataRateRange
