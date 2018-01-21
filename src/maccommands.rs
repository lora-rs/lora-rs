// Copyright (c) 2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use std::collections::HashMap;

/// MacCommand represents the enumeration of all LoRaWAN MACCommands.
#[derive(Debug, PartialEq)]
pub enum MacCommand<'a> {
    LinkCheckReq(LinkCheckReqPayload),
    LinkCheckAns(LinkCheckAnsPayload<'a>),
    LinkADRReq(LinkADRReqPayload<'a>),
    LinkADRAns(LinkADRAnsPayload),
    DutyCycleReq(DutyCycleReqPayload),
    DutyCycleAns(DutyCycleAnsPayload),
    RXParamSetupReq(RXParamSetupReqPayload<'a>),
    RXParamSetupAns(RXParamSetupAnsPayload),
    DevStatusReq(DevStatusReqPayload),
    DevStatusAns(DevStatusAnsPayload<'a>),
    NewChannelReq(NewChannelReqPayload<'a>),
    NewChannelAns(NewChannelAnsPayload),
    RXTimingSetupReq(RXTimingSetupReqPayload),
    RXTimingSetupAns(RXTimingSetupAnsPayload),
}

type NewMacCommandFn = Box<for<'b> Fn(&'b [u8]) -> Result<(MacCommand<'b>, usize), String>>;

// Helper macro for adding all the default mac command types to a HashMap.
// See https://doc.rust-lang.org/std/macro.vec.html if you want it to work
// without the comma after the last item.
macro_rules! mac_cmds_map  {
    ( $( $x:ident ,)*) => {{
        {
            let mut tmp: HashMap<(u8, bool), NewMacCommandFn> = HashMap::new();
            $(
                tmp.insert(($x::cid(), $x::uplink()), Box::new($x::new));
            )*
            tmp
        }
    }};
}

fn format_error(expected: usize, actual: usize) -> String {
    format!("not enough bytes to read: needs {}, given {}", expected, actual)
}

macro_rules! new_mac_cmd_helper  {
    ( $name:ident, $type:ident, $data:ident, 0 ) => {{
        Ok((MacCommand::$name($type()), 0))
    }};
    ( $name:ident, $type:ident, $data:ident, 1 ) => {{
        if $data.len() < 1 {
            return Err(format_error(1, $data.len()));
        }
        Ok((MacCommand::$name($type($data[0])), 1))
    }};
    ( $name:ident, $type:ident, $data:ident, $len:expr ) => {{
        {
            if $data.len() < $len {
                return Err(format_error($len, $data.len()));
            }
            let payload = array_ref![&$data[..$len], 0, $len];
            Ok((MacCommand::$name($type(payload)), $len))
        }
    }};
}

pub fn parse_mac_commands<'a>(data: &'a [u8], uplink: bool) -> Result<Vec<MacCommand<'a>>, String> {
    let cid_to_parser =
        mac_cmds_map![
            LinkCheckReqPayload,
            LinkADRAnsPayload,
            LinkADRReqPayload,
            LinkADRAnsPayload,
            DutyCycleReqPayload,
            DutyCycleAnsPayload,
            RXParamSetupReqPayload,
            RXParamSetupAnsPayload,
            DevStatusReqPayload,
            DevStatusAnsPayload,
            NewChannelReqPayload,
            NewChannelAnsPayload,
            RXTimingSetupReqPayload,
            RXTimingSetupAnsPayload,
        ];
    println!("{:?}", cid_to_parser.keys());
    let mut i = 0;
    let mut res = Vec::new();
    while i < data.len() {
        if let Some(f) = cid_to_parser.get(&(data[i], uplink)) {
            i += 1;
            let t = f(&data[i..])?;
            res.push(t.0);
            i += t.1;
        } else {
            break;
        }
    }
    Ok(res)
}

/// LinkCheckReqPayload represents the LinkCheckReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkCheckReqPayload();

impl LinkCheckReqPayload {
    pub fn cid() -> u8 {
        0x02
    }

    pub fn new<'a>(_: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(LinkCheckReq, LinkCheckReqPayload, data, 0)
    }

    pub fn uplink() -> bool {
        true
    }
}

/// LinkCheckAnsPayload represents the LinkCheckAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkCheckAnsPayload<'a>(&'a [u8; 2]);

impl<'a> LinkCheckAnsPayload<'a> {
    pub fn cid() -> u8 {
        0x02
    }

    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(LinkCheckAns, LinkCheckAnsPayload, data, 2)
    }

    pub fn uplink() -> bool {
        false
    }

    pub fn margin(&self) -> u8 {
        self.0[0]
    }

    pub fn gateway_count(&self) -> u8 {
        self.0[1]
    }
}

impl<'a> From<&'a [u8; 2]> for LinkCheckAnsPayload<'a> {
    fn from(v: &'a [u8; 2]) -> Self {
        LinkCheckAnsPayload(v)
    }
}

/// LinkADRReqPayload represents the LinkADRReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkADRReqPayload<'a>(&'a [u8; 4]);

impl<'a> LinkADRReqPayload<'a> {
    pub fn cid() -> u8 {
        0x03
    }

    pub fn uplink() -> bool {
        false
    }

    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(LinkADRReq, LinkADRReqPayload, data, 4)
    }

    pub fn data_rate(&self) -> u8 {
        self.0[0] >> 4
    }

    pub fn tx_power(&self) -> u8 {
        self.0[0] & 0x0f
    }

    pub fn channel_mask(&self) -> ChannelMask {
        ChannelMask::new_from_raw(&self.0[1..3])
    }

    pub fn redundancy(&self) -> Redundancy {
        Redundancy::new(self.0[3])
    }
}

/// ChannelMask represents the ChannelMask from LoRaWAN.
#[derive(Debug, PartialEq)]
pub struct ChannelMask<'a>(&'a [u8; 2]);

impl<'a> ChannelMask<'a> {
    pub fn new<'b>(data: &'b [u8]) -> Result<ChannelMask<'b>, String> {
        if data.len() < 2 {
            let msg =
                format!(
                "not enough bytes to read: needs {}, given {}",
                2,
                data.len(),
                );
            return Err(msg);
        }
        Ok(Self::new_from_raw(data))
    }

    pub fn new_from_raw<'b>(data: &'b [u8]) -> ChannelMask<'b> {
        let payload = array_ref![&data[..2], 0, 2];
        ChannelMask(payload)
    }
}

/// Redundancy represents the ChannelMask from LoRaWAN.
#[derive(Debug, PartialEq)]
pub struct Redundancy(u8);

impl Redundancy {
    pub fn new(data: u8) -> Redundancy {
        Redundancy(data)
    }

    pub fn channel_mask_control(&self) -> u8 {
        0
    }

    pub fn number_of_transmissions(&self) -> u8 {
        0
    }
}

/// LinkADRAnsPayload represents the LinkADRAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkADRAnsPayload(u8);

impl LinkADRAnsPayload {
    pub fn cid() -> u8 {
        0x03
    }

    pub fn uplink() -> bool {
        true
    }

    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(LinkADRAns, LinkADRAnsPayload, data, 1)
    }

    pub fn payload_size() -> usize {
        4
    }

    pub fn channel_mask_ack(&self) -> bool {
        self.0 & 0x01 != 0
    }

    pub fn data_rate_ack(&self) -> bool {
        self.0 & 0x02 != 0
    }

    pub fn powert_ack(&self) -> bool {
        self.0 & 0x04 != 0
    }
}

/// DutyCycleReqPayload represents the DutyCycleReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DutyCycleReqPayload(u8);

impl DutyCycleReqPayload {
    pub fn cid() -> u8 {
        0x04
    }

    pub fn uplink() -> bool {
        false
    }

    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(DutyCycleReq, DutyCycleReqPayload, data, 1)
    }

    pub fn max_duty_cycle_raw(&self) -> u8 {
        self.0 & 0x0f
    }

    pub fn max_duty_cycle(&self) -> f32 {
        let divisor = 1 << self.max_duty_cycle_raw();
        1.0 / (divisor as f32)
    }
}

/// DutyCycleAnsPayload represents the DutyCycleAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DutyCycleAnsPayload();

impl DutyCycleAnsPayload {
    pub fn cid() -> u8 {
        0x04
    }

    pub fn uplink() -> bool {
        true
    }

    pub fn new<'b>(_: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(DutyCycleAns, DutyCycleAnsPayload, data, 0)
    }
}

/// RXParamSetupReqPayload represents the RXParamSetupReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXParamSetupReqPayload<'a>(&'a [u8; 4]);

impl<'a> RXParamSetupReqPayload<'a> {
    pub fn cid() -> u8 {
        0x05
    }

    pub fn uplink() -> bool {
        false
    }

    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(RXParamSetupReq, RXParamSetupReqPayload, data, 4)
    }

    pub fn dl_settings(&self) -> DLSettings {
        DLSettings::new(self.0[0])
    }

    pub fn frequency(&self) -> Frequency {
        Frequency::new_from_raw(&self.0[1..])
    }
}

/// DLSettings represents LoRaWAN DLSettings.
#[derive(Debug, PartialEq)]
pub struct DLSettings(u8);

impl DLSettings {
    pub fn new(byte: u8) -> DLSettings {
        DLSettings(byte)
    }

    pub fn rx1_dr_offset(&self) -> u8 {
        self.0 >> 4 & 0x07
    }

    pub fn rx2_data_rate(&self) -> u8 {
        self.0 & 0x0f
    }

    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for DLSettings {
    fn from(v: u8) -> Self {
        DLSettings(v)
    }
}

/// Frequency represents a channel's frequency.
#[derive(Debug, PartialEq)]
pub struct Frequency<'a>(&'a [u8]);

impl<'a> Frequency<'a> {
    pub fn new_from_raw(bytes: &'a [u8]) -> Frequency {
        Frequency(bytes)
    }

    pub fn new(bytes: &'a [u8]) -> Option<Frequency> {
        if bytes.len() != 3 {
            return None;
        }

        Some(Frequency(bytes))
    }

    pub fn value(&self) -> u32 {
        (((self.0[2] as u32) << 16) + ((self.0[1] as u32) << 8) + (self.0[0] as u32)) * 100
    }
}

/// RXParamSetupAnsPayload represents the RXParamSetupAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXParamSetupAnsPayload(u8);

impl RXParamSetupAnsPayload {
    pub fn cid() -> u8 {
        0x05
    }

    pub fn uplink() -> bool {
        true
    }

    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(RXParamSetupAns, RXParamSetupAnsPayload, data, 1)
    }

    pub fn channel_ack(&self) -> bool {
        (self.0 & 0x01) != 0
    }

    pub fn rx2_data_rate_ack(&self) -> bool {
        (self.0 & 0x02) != 0
    }

    pub fn rx1_dr_offset_ack(&self) -> bool {
        (self.0 & 0x04) != 0
    }
}

/// DevStatusReqPayload represents the DevStatusReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DevStatusReqPayload();

impl DevStatusReqPayload {
    pub fn cid() -> u8 {
        0x06
    }

    pub fn uplink() -> bool {
        false
    }

    pub fn new<'b>(_: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(DevStatusReq, DevStatusReqPayload, data, 0)
    }
}

/// DevStatusAnsPayload represents the DevStatusAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DevStatusAnsPayload<'a>(&'a [u8; 2]);

impl<'a> DevStatusAnsPayload<'a> {
    pub fn cid() -> u8 {
        0x06
    }

    pub fn uplink() -> bool {
        true
    }

    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(DevStatusAns, DevStatusAnsPayload, data, 2)
    }

    pub fn battery(&self) -> u8 {
        self.0[0]
    }

    pub fn margin(&self) -> i8 {
        ((self.0[1] << 2) as i8) >> 2
    }
}

/// NewChannelReqPayload represents the NewChannelReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct NewChannelReqPayload<'a>(&'a [u8; 5]);

impl<'a> NewChannelReqPayload<'a> {
    pub fn cid() -> u8 {
        0x07
    }

    pub fn uplink() -> bool {
        false
    }

    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(NewChannelReq, NewChannelReqPayload, data, 5)
    }

    pub fn channel_index(&self) -> u8 {
        self.0[0]
    }

    pub fn frequency(&self) -> Frequency {
        Frequency::new_from_raw(&self.0[1..4])
    }

    pub fn data_rate_range(&self) -> DataRateRange {
        DataRateRange::new(self.0[4])
    }
}

/// DataRateRange represents LoRaWAN DataRateRange.
#[derive(Debug, PartialEq)]
pub struct DataRateRange(u8);

impl DataRateRange {
    pub fn new(byte: u8) -> DataRateRange {
        DataRateRange(byte)
    }

    pub fn max_data_rate(&self) -> u8 {
        self.0 >> 4
    }

    pub fn min_data_range(&self) -> u8 {
        self.0 & 0x0f
    }

    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for DataRateRange {
    fn from(v: u8) -> Self {
        DataRateRange(v)
    }
}

/// NewChannelAnsPayload represents the NewChannelAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct NewChannelAnsPayload(u8);

impl NewChannelAnsPayload {
    pub fn cid() -> u8 {
        0x07
    }

    pub fn uplink() -> bool {
        true
    }

    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(NewChannelAns, NewChannelAnsPayload, data, 1)
    }

    pub fn data_rate_range_ack(&self) -> bool {
        self.0 & 0x02 != 0
    }

    pub fn channel_freq_ack(&self) -> bool {
        self.0 & 0x01 != 0
    }
}

/// RXTimingSetupReqPayload represents the RXTimingSetupReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXTimingSetupReqPayload(u8);

impl RXTimingSetupReqPayload {
    pub fn cid() -> u8 {
        0x08
    }

    pub fn uplink() -> bool {
        false
    }

    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(RXTimingSetupReq, RXTimingSetupReqPayload, data, 1)
    }

    pub fn delay(&self) -> u8 {
        self.0 & 0x0f
    }
}

/// RXTimingSetupAnsPayload represents the RXTimingSetupAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXTimingSetupAnsPayload();

impl RXTimingSetupAnsPayload {
    pub fn cid() -> u8 {
        0x08
    }

    pub fn uplink() -> bool {
        true
    }

    pub fn new<'a>(_data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(RXTimingSetupAns, RXTimingSetupAnsPayload, data, 0)
    }
}
