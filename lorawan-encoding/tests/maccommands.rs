use lorawan::maccommandcreator::*;
use lorawan::maccommands::*;

macro_rules! test_helper {
    ( $cmd:ident, $data:ident, $name:ident, $type:ident, $size:expr, $( ( $method:ident, $val:expr ) ,)*) => {{
        {
            assert!($type::new(&[]).is_err());
            let res = $type::new(&$data[..]).unwrap();
            assert_eq!($type::len(), $size);
            $(
                assert_eq!(res.$method(), $val);
            )*
        }
    }};

    ( $cmd:ident, $name:ident, $type:ident ) => {{
        {
            let data = [];
            let mc = $cmd::$name($type::new(&data[..]));
            assert_eq!(mc.len(), 0);
        }
    }};
}

#[test]
fn test_link_check_req_new() {
    test_helper!(UplinkMacCommand, LinkCheckReq, LinkCheckReqPayload);
}

#[test]
fn test_link_check_ans_new() {
    let data = [0xa, 0x0f];
    test_helper!(
        DownlinkMacCommand,
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
    let data = [0x12, 0x04, 0x00, 0x45];
    let expected_channel_mask = ChannelMask::new(&[0x04, 0x00]).unwrap();
    test_helper!(
        DownlinkMacCommand,
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
    assert!(LinkADRReqPayload::new(&[]).is_err());
    for (v, e_power, e_dr, e_cm, e_ack) in &examples {
        let laa = LinkADRAnsPayload::new(&v[..]).unwrap();
        assert_eq!(LinkADRAnsPayload::len(), 1);
        assert_eq!(laa.channel_mask_ack(), *e_power);
        assert_eq!(laa.data_rate_ack(), *e_dr);
        assert_eq!(laa.powert_ack(), *e_cm);
        assert_eq!(laa.ack(), *e_ack);
    }
}

#[test]
fn test_duty_cycle_req_new() {
    #![allow(clippy::float_cmp)]
    let data = [0x02];
    test_helper!(
        DownlinkMacCommand,
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
    test_helper!(UplinkMacCommand, DutyCycleAns, DutyCycleAnsPayload);
}

#[test]
fn test_rx_param_setup_req_new() {
    let data = [0x3b, 0x01, 0x02, 0x04];
    test_helper!(
        DownlinkMacCommand,
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
    assert!(RXParamSetupAnsPayload::new(&[]).is_err());
    for (v, e_ch, e_rx2_dr, e_rx1_dr_offset, e_ack) in &examples {
        let psa = RXParamSetupAnsPayload::new(&v[..]).unwrap();
        assert_eq!(RXParamSetupAnsPayload::len(), 1);
        assert_eq!(psa.channel_ack(), *e_ch);
        assert_eq!(psa.rx2_data_rate_ack(), *e_rx2_dr);
        assert_eq!(psa.rx1_dr_offset_ack(), *e_rx1_dr_offset);
        assert_eq!(psa.ack(), *e_ack);
    }
}

#[test]
fn test_dev_status_req() {
    test_helper!(DownlinkMacCommand, DevStatusReq, DevStatusReqPayload);
}

#[test]
fn test_dev_status_ans() {
    let data = [0xfe, 0x3f];
    test_helper!(
        DownlinkMacCommand,
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
    let data = [0x03, 0x01, 0x02, 0x04, 0xa5];
    test_helper!(
        DownlinkMacCommand,
        data,
        NewChannelReq,
        NewChannelReqPayload,
        5,
        (channel_index, 3),
        (frequency, Frequency::new_from_raw(&data[1..4])),
        (data_rate_range, DataRateRange::new_from_raw(data[4])),
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
    assert!(NewChannelAnsPayload::new(&[]).is_err());
    for (v, e_ch_freq, e_drr, e_ack) in &examples {
        let nca = NewChannelAnsPayload::new(&v[..]).unwrap();
        assert_eq!(NewChannelAnsPayload::len(), 1);
        assert_eq!(nca.data_rate_range_ack(), *e_drr);
        assert_eq!(nca.channel_freq_ack(), *e_ch_freq);
        assert_eq!(nca.ack(), *e_ack);
    }
}

#[test]
fn test_rx_timing_setup_req() {
    let data = [0x02];
    test_helper!(UplinkMacCommand, data, RXTimingSetupReq, RXTimingSetupReqPayload, 1, (delay, 2),);
}

#[test]
fn test_rx_timing_setup_ans() {
    test_helper!(UplinkMacCommand, RXTimingSetupAns, RXTimingSetupAnsPayload);
}

#[test]
fn test_tx_param_setup_req() {
    let data = [0b011110];
    test_helper!(
        UplinkMacCommand,
        data,
        TXParamSetupReq,
        TXParamSetupReqPayload,
        1,
        (downlink_dwell_time, false),
        (uplink_dwell_time, true),
        (max_eirp, 33),
    );
}

#[test]
fn test_tx_param_setup_ans() {
    test_helper!(UplinkMacCommand, TXParamSetupAns, TXParamSetupAnsPayload);
}

#[test]
fn test_dl_channel_req() {
    let data = [1, 2, 3, 4];
    test_helper!(
        UplinkMacCommand,
        data,
        DlChannelReq,
        DlChannelReqPayload,
        4,
        (channel_index, 1),
        (frequency, Frequency::new_from_raw(&data[1..4])),
    );
}

#[test]
fn test_dl_channel_ans() {
    let data = [0x3];
    test_helper!(
        UplinkMacCommand,
        data,
        DlChannelAns,
        DlChannelAnsPayload,
        1,
        (channel_freq_ack, true),
        (uplink_freq_ack, true),
    );
}

#[test]
fn test_parse_mac_commands_empty_downlink() {
    assert_eq!(parse_downlink_mac_commands(&[]).count(), 0);
}

#[test]
fn test_device_time_req() {
    test_helper!(UplinkMacCommand, DeviceTimeReq, DeviceTimeReqPayload);
}
#[test]
fn test_device_time_ans() {
    let data = [0x1, 0x2, 0x3, 0x4, 0x5];
    test_helper!(
        UplinkMacCommand,
        data,
        DeviceTimeAns,
        DeviceTimeAnsPayload,
        5,
        (seconds, 16909060),
        (nano_seconds, 0x5 * 3906250),
    );
}

#[test]
fn test_parse_mac_commands_empty_uplink() {
    assert_eq!(parse_uplink_mac_commands(&[]).count(), 0);
}

#[test]
fn test_parse_mac_commands_with_multiple_cmds() {
    let data = mac_cmds_payload();
    let mut commands = parse_uplink_mac_commands(&data[..]);
    assert_eq!(commands.next(), Some(UplinkMacCommand::LinkCheckReq(LinkCheckReqPayload())));
    let expected = UplinkMacCommand::LinkADRAns(LinkADRAnsPayload::new(&data[2..]).unwrap());
    assert_eq!(commands.next(), Some(expected));
}

#[test]
fn test_parse_mac_commands_with_multiple_cmds_with_payloads() {
    let data = [3, 0, 0, 0, 112, 3, 0, 0, 255, 0];
    let mut commands = parse_downlink_mac_commands(&data);

    assert_eq!(
        commands.next(),
        Some(DownlinkMacCommand::LinkADRReq(LinkADRReqPayload::new(&[0, 0, 0, 112]).unwrap()))
    );

    assert_eq!(
        commands.next(),
        Some(DownlinkMacCommand::LinkADRReq(LinkADRReqPayload::new(&[0, 0, 255, 0]).unwrap()))
    );
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
    let data = [0x03, 0x10];
    let mut expected = [false; 16];
    expected[0] = true;
    expected[1] = true;
    expected[12] = true;
    let chan_mask = ChannelMask::<2>::new(&data[..]);
    assert!(chan_mask.is_ok());
    assert_eq!(&chan_mask.unwrap().statuses::<16>()[..], &expected[..]);
}

#[test]
fn test_channel_mask_enable_and_disable_channel() {
    let data = [0x00, 0x00];
    let mut expected = [false; 16];
    let mut chan_mask = ChannelMask::<2>::new(&data[..]).unwrap();
    chan_mask.set_channel(15, true);
    expected[15] = true;
    assert_eq!(&chan_mask.statuses::<16>()[..], &expected[..]);
    chan_mask.set_channel(15, false);
    expected[15] = false;
    assert_eq!(&chan_mask.statuses::<16>()[..], &expected[..]);
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
    assert!(Frequency::new(&[]).is_none());
}

#[test]
fn test_frequency_value() {
    let data = frequency_payload();
    let freq = Frequency::new(&data[..]);
    assert!(freq.is_some());
    assert_eq!(freq.unwrap().value(), 26_265_700);
}

fn frequency_payload() -> [u8; 3] {
    [0x01, 0x02, 0x04]
}

#[test]
fn test_data_rate_range() {
    let drr_raw = DataRateRange::new(0xa5);
    assert!(drr_raw.is_ok());
    let drr = drr_raw.unwrap();
    assert_eq!(drr.max_data_rate(), 0x0a);
    assert_eq!(drr.min_data_rate(), 0x05);
}

#[test]
fn test_data_rate_range_inversed_min_and_max() {
    let drr = DataRateRange::new(0x5a);
    assert!(drr.is_err());
}

#[test]
fn test_data_rate_range_max_equals_min() {
    let drr_raw = DataRateRange::new(0x55);
    assert!(drr_raw.is_ok());
}

#[test]
fn test_mac_commands_len_with_creators() {
    let rx_timing_setup_req = RXTimingSetupReqCreator::new();
    let dev_status_req = DevStatusReqCreator::new();
    let cmds: Vec<&dyn SerializableMacCommand> = vec![&rx_timing_setup_req, &dev_status_req];

    assert_eq!(mac_commands_len(&cmds[..]), 3);
}

#[test]
fn test_mac_commands_len_with_mac_cmds() {
    let rx_timing_setup_req =
        DownlinkMacCommand::RXTimingSetupReq(RXTimingSetupReqPayload::new(&[0x02]).unwrap());
    let dev_status_ans =
        UplinkMacCommand::DevStatusAns(DevStatusAnsPayload::new(&[0xfe, 0x3f]).unwrap());
    let cmds: Vec<&dyn SerializableMacCommand> = vec![&rx_timing_setup_req, &dev_status_ans];

    assert_eq!(mac_commands_len(&cmds[..]), 5);
}
