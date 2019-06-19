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
    LinkADRAns(LinkADRAnsPayload<'a>),
    DutyCycleReq(DutyCycleReqPayload<'a>),
    DutyCycleAns(DutyCycleAnsPayload),
    RXParamSetupReq(RXParamSetupReqPayload<'a>),
    RXParamSetupAns(RXParamSetupAnsPayload<'a>),
    DevStatusReq(DevStatusReqPayload),
    DevStatusAns(DevStatusAnsPayload<'a>),
    NewChannelReq(NewChannelReqPayload<'a>),
    NewChannelAns(NewChannelAnsPayload<'a>),
    RXTimingSetupReq(RXTimingSetupReqPayload<'a>),
    RXTimingSetupAns(RXTimingSetupAnsPayload),
}

impl<'a> MacCommand<'a> {
    #![allow(clippy::clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match *self {
            MacCommand::LinkCheckReq(_) => LinkCheckReqPayload::len(),
            MacCommand::LinkCheckAns(_) => LinkCheckAnsPayload::len(),
            MacCommand::LinkADRReq(_) => LinkADRReqPayload::len(),
            MacCommand::LinkADRAns(_) => LinkADRAnsPayload::len(),
            MacCommand::DutyCycleReq(_) => DutyCycleReqPayload::len(),
            MacCommand::DutyCycleAns(_) => DutyCycleAnsPayload::len(),
            MacCommand::RXParamSetupReq(_) => RXParamSetupReqPayload::len(),
            MacCommand::RXParamSetupAns(_) => RXParamSetupAnsPayload::len(),
            MacCommand::DevStatusReq(_) => DevStatusReqPayload::len(),
            MacCommand::DevStatusAns(_) => DevStatusAnsPayload::len(),
            MacCommand::NewChannelReq(_) => NewChannelReqPayload::len(),
            MacCommand::NewChannelAns(_) => NewChannelAnsPayload::len(),
            MacCommand::RXTimingSetupReq(_) => RXTimingSetupReqPayload::len(),
            MacCommand::RXTimingSetupAns(_) => RXTimingSetupAnsPayload::len(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        match *self {
            MacCommand::LinkCheckReq(_) => &[],
            MacCommand::LinkCheckAns(ref v) => &v.0[..],
            MacCommand::LinkADRReq(ref v) => &v.0[..],
            MacCommand::LinkADRAns(ref v) => &v.0[..],
            MacCommand::DutyCycleReq(ref v) => &v.0[..],
            MacCommand::DutyCycleAns(_) => &[],
            MacCommand::RXParamSetupReq(ref v) => &v.0[..],
            MacCommand::RXParamSetupAns(ref v) => &v.0[..],
            MacCommand::DevStatusReq(_) => &[],
            MacCommand::DevStatusAns(ref v) => &v.0[..],
            MacCommand::NewChannelReq(ref v) => &v.0[..],
            MacCommand::NewChannelAns(ref v) => &v.0[..],
            MacCommand::RXTimingSetupReq(ref v) => &v.0[..],
            MacCommand::RXTimingSetupAns(_) => &[],
        }
    }
}

pub trait SerializableMacCommand {
    fn payload_bytes(&self) -> &[u8];
    fn cid(&self) -> u8;
    fn payload_len(&self) -> usize;
}

impl<'a> SerializableMacCommand for MacCommand<'a> {
    fn payload_bytes(&self) -> &[u8] {
        self.bytes()
    }

    fn cid(&self) -> u8 {
        match *self {
            MacCommand::LinkCheckReq(_) => LinkCheckReqPayload::cid(),
            MacCommand::LinkCheckAns(_) => LinkCheckAnsPayload::cid(),
            MacCommand::LinkADRReq(_) => LinkADRReqPayload::cid(),
            MacCommand::LinkADRAns(_) => LinkADRAnsPayload::cid(),
            MacCommand::DutyCycleReq(_) => DutyCycleReqPayload::cid(),
            MacCommand::DutyCycleAns(_) => DutyCycleAnsPayload::cid(),
            MacCommand::RXParamSetupReq(_) => RXParamSetupReqPayload::cid(),
            MacCommand::RXParamSetupAns(_) => RXParamSetupAnsPayload::cid(),
            MacCommand::DevStatusReq(_) => DevStatusReqPayload::cid(),
            MacCommand::DevStatusAns(_) => DevStatusAnsPayload::cid(),
            MacCommand::NewChannelReq(_) => NewChannelReqPayload::cid(),
            MacCommand::NewChannelAns(_) => NewChannelAnsPayload::cid(),
            MacCommand::RXTimingSetupReq(_) => RXTimingSetupReqPayload::cid(),
            MacCommand::RXTimingSetupAns(_) => RXTimingSetupAnsPayload::cid(),
        }
    }

    fn payload_len(&self) -> usize {
        self.len()
    }
}

/// Calculates the len in bytes of a sequence of mac commands, including th CIDs.
pub fn mac_commands_len(cmds: &[&SerializableMacCommand]) -> usize {
    cmds.iter().map(|mc| mc.payload_len() + 1).sum()
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
                tmp.insert(($x::cid(), $x::uplink()), Box::new($x::new_as_mac_cmd));
            )*
            tmp
        }
    }};
}

fn format_error(expected: usize, actual: usize) -> String {
    format!(
        "not enough bytes to read: needs {}, given {}",
        expected, actual
    )
}

macro_rules! new_mac_cmd_helper {
    ($name:ident, $type:ident,0) => {
        pub fn new_as_mac_cmd(_: &[u8]) -> Result<(MacCommand, usize), String> {
            Ok((MacCommand::$name($type()), 0))
        }
    };
    ($name:ident, $type:ident, $len:expr) => {
        pub fn new_as_mac_cmd(data: &[u8]) -> Result<(MacCommand, usize), String> {
            #![allow(clippy::range_plus_one)]
            if let Err(err) = Self::can_build_from(data) {
                return Err(err);
            }
            let payload = array_ref![&data[..$len], 0, $len];
            Ok((MacCommand::$name($type(payload)), $len))
        }
    };
}

macro_rules! create_type_const_fn {
    (can_build_from) => {
        pub fn can_build_from(bytes: &[u8]) -> Result<(), String> {
            if bytes.len() < Self::len() {
                return Err(format_error(Self::len(), bytes.len()));
            }
            Ok(())
        }
    };

    ($name:ident, $type:ty, $val:expr) => {
        pub fn $name() -> $type {
            $val
        }
    };
}

macro_rules! create_ack_fn {
    ( $fn_name:ident, $offset:expr ) => (
        pub fn $fn_name(&self) -> bool {
            self.0[0] & (0x01 << $offset) != 0
        }
    )
}

macro_rules! create_value_reader_fn {
    ( $fn_name:ident, $index:expr ) => (
        pub fn $fn_name(&self) -> u8 {
            self.0[$index]
        }
    )
}

/// Parses bytes to mac commands if possible.
///
/// Could return error if some values are out of range or the payload does not end at mac command
/// boundry.
/// # Argument
///
/// * bytes - the data from which the MAC commands are to be built.
/// * uplink - whether the packet is uplink or downlink.
///
/// # Examples
///
/// ```
/// let mut data = vec![0x02, 0x03, 0x00];
/// let mac_cmds = lorawan::maccommands::parse_mac_commands(&data[..], true);
/// ```
pub fn parse_mac_commands<'a>(
    bytes: &'a [u8],
    uplink: bool,
) -> Result<Vec<MacCommand<'a>>, String> {
    let cid_to_parser = mac_cmds_map![
        LinkCheckReqPayload,
        LinkCheckAnsPayload,
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
    let mut i = 0;
    let mut res = Vec::new();
    while i < bytes.len() {
        if let Some(f) = cid_to_parser.get(&(bytes[i], uplink)) {
            i += 1;
            let t = f(&bytes[i..])?;
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
    /// Command identifier for LinkCheckReqPayload.
    create_type_const_fn!(cid, u8, 0x02);

    /// Whether LinkCheckReqPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, true);

    /// The len
    create_type_const_fn!(len, usize, 0);

    /// Check if the bytes can be used to create LinkCheckReqPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new LinkCheckReqPayload.
    new_mac_cmd_helper!(LinkCheckReq, LinkCheckReqPayload, 0);
}

/// LinkCheckAnsPayload represents the LinkCheckAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkCheckAnsPayload<'a>(&'a [u8; 2]);

impl<'a> LinkCheckAnsPayload<'a> {
    /// Command identifier for LinkCheckAnsPayload.
    create_type_const_fn!(cid, u8, 0x02);

    /// Whether LinkCheckAnsPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, false);

    /// The len
    create_type_const_fn!(len, usize, 2);

    /// Check if the bytes can be used to create LinkCheckAnsPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new LinkCheckAnsPayload from the provided data.
    new_mac_cmd_helper!(LinkCheckAns, LinkCheckAnsPayload, 2);

    /// The link margin in dB of the last successfully received LinkCheckReq command.
    create_value_reader_fn!(margin, 0);

    /// The number of gateways that successfully received the last LinkCheckReq command.
    create_value_reader_fn!(gateway_count, 1);
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
    /// Command identifier for LinkADRReqPayload.
    create_type_const_fn!(cid, u8, 0x03);

    /// Whether LinkADRReqPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, false);

    /// The len
    create_type_const_fn!(len, usize, 4);

    /// Check if the bytes can be used to create LinkADRReqPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new LinkADRReqPayload from the provided data.
    new_mac_cmd_helper!(LinkADRReq, LinkADRReqPayload, 4);

    /// Data Rate that the device should use for its next transmissions.
    pub fn data_rate(&self) -> u8 {
        self.0[0] >> 4
    }

    /// TX Power that the device should use for its next transmissions.
    pub fn tx_power(&self) -> u8 {
        self.0[0] & 0x0f
    }

    /// Usable channels for next trasnmissions.
    pub fn channel_mask(&self) -> ChannelMask {
        ChannelMask::new_from_raw(&self.0[1..3])
    }

    /// Provides information how channel mask is to be interpreted and how many times each message
    /// should be repeated.
    pub fn redundancy(&self) -> Redundancy {
        Redundancy::new(self.0[3])
    }
}

/// ChannelMask represents the ChannelMask from LoRaWAN.
#[derive(Debug, PartialEq)]
pub struct ChannelMask([u8; 2]);

impl ChannelMask {
    /// Constructs a new ChannelMask from the provided data.
    pub fn new(data: &[u8]) -> Result<ChannelMask, String> {
        if data.len() < 2 {
            return Err(format_error(2, data.len()));
        }
        Ok(Self::new_from_raw(data))
    }

    /// Constructs a new ChannelMask from the provided data, without verifying if they are
    /// admissible.
    ///
    /// Improper use of this method could lead to panic during runtime!
    pub fn new_from_raw(data: &[u8]) -> ChannelMask {
        let payload = [data[0], data[1]];
        ChannelMask(payload)
    }

    fn channel_enabled(&self, index: usize) -> bool {
        self.0[index >> 3] & (1 << (index & 0x07)) != 0
    }

    /// Verifies if a given channel is enabled.
    pub fn is_enabled(&self, index: usize) -> Result<bool, String> {
        if index > 15 {
            return Err(String::from("index should be between 0 and 15"));
        }
        Ok(self.channel_enabled(index))
    }

    /// Provides information for each of the 16 channels if they are enabled.
    pub fn statuses(&self) -> Vec<bool> {
        (0..16).map(|v| self.channel_enabled(v)).collect()
    }
}

impl From<[u8; 2]> for ChannelMask {
    fn from(v: [u8; 2]) -> Self {
        ChannelMask(v)
    }
}

impl AsRef<[u8]> for ChannelMask {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

/// Redundancy represents the LinkADRReq Redundancy from LoRaWAN.
#[derive(Debug, PartialEq)]
pub struct Redundancy(u8);

impl Redundancy {
    /// Constructs a new Redundancy from the provided data.
    pub fn new(data: u8) -> Redundancy {
        Redundancy(data)
    }

    /// Controls the interpretation of the previously defined ChannelMask bit mask.
    pub fn channel_mask_control(&self) -> u8 {
        (self.0 >> 4) & 0x07
    }

    /// How many times each message should be repeated.
    pub fn number_of_transmissions(&self) -> u8 {
        self.0 & 0x0f
    }

    /// The integer value of the Redundancy.
    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for Redundancy {
    fn from(v: u8) -> Self {
        Redundancy(v)
    }
}

/// LinkADRAnsPayload represents the LinkADRAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkADRAnsPayload<'a>(&'a [u8; 1]);

impl<'a> LinkADRAnsPayload<'a> {
    /// Command identifier for LinkADRAnsPayload.
    create_type_const_fn!(cid, u8, 0x03);

    /// Whether LinkADRAnsPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, true);

    /// The len
    create_type_const_fn!(len, usize, 1);

    /// Check if the bytes can be used to create LinkADRAnsPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new LinkADRAnsPayload from the provided data.
    new_mac_cmd_helper!(LinkADRAns, LinkADRAnsPayload, 1);

    /// Whether the channel mask change was applied successsfully.
    create_ack_fn!(channel_mask_ack, 0);

    /// Whether the data rate change was applied successsfully.
    create_ack_fn!(data_rate_ack, 1);

    /// Whether the power change was applied successsfully.
    create_ack_fn!(powert_ack, 2);

    /// Whether the device has accepted the new parameters or not.
    pub fn ack(&self) -> bool {
        self.0[0] == 0x07
    }
}

/// DutyCycleReqPayload represents the DutyCycleReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DutyCycleReqPayload<'a>(&'a [u8; 1]);

impl<'a> DutyCycleReqPayload<'a> {
    /// Command identifier for DutyCycleReqPayload.
    create_type_const_fn!(cid, u8, 0x04);

    /// Whether DutyCycleReqPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, false);

    /// The len
    create_type_const_fn!(len, usize, 1);

    /// Check if the bytes can be used to create DutyCycleReqPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new DutyCycleReqPayload from the provided data.
    new_mac_cmd_helper!(DutyCycleReq, DutyCycleReqPayload, 1);

    /// Integer value of the max duty cycle field.
    pub fn max_duty_cycle_raw(&self) -> u8 {
        self.0[0] & 0x0f
    }

    /// Value of the max duty cycle field as portion of time (ex: 0.5).
    pub fn max_duty_cycle(&self) -> f32 {
        let divisor = 1 << self.max_duty_cycle_raw();
        1.0 / (divisor as f32)
    }
}

/// DutyCycleAnsPayload represents the DutyCycleAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DutyCycleAnsPayload();

impl DutyCycleAnsPayload {
    /// Command identifier for DutyCycleAnsPayload.
    create_type_const_fn!(cid, u8, 0x04);

    /// Whether DutyCycleAnsPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, true);

    /// The len
    create_type_const_fn!(len, usize, 0);

    /// Check if the bytes can be used to create DutyCycleAnsPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new DutyCycleAnsPayload from the provided data.
    new_mac_cmd_helper!(DutyCycleAns, DutyCycleAnsPayload, 0);
}

/// RXParamSetupReqPayload represents the RXParamSetupReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXParamSetupReqPayload<'a>(&'a [u8; 4]);

impl<'a> RXParamSetupReqPayload<'a> {
    /// Command identifier for RXParamSetupReqPayload.
    create_type_const_fn!(cid, u8, 0x05);

    /// Whether RXParamSetupReqPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, false);

    /// The len
    create_type_const_fn!(len, usize, 4);

    /// Check if the bytes can be used to create RXParamSetupReqPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new RXParamSetupReqPayload from the provided data.
    new_mac_cmd_helper!(RXParamSetupReq, RXParamSetupReqPayload, 4);

    /// Downlink settings - namely rx1_dr_offset and rx2_data_rate.
    pub fn dl_settings(&self) -> DLSettings {
        DLSettings::new(self.0[0])
    }

    /// RX2 frequency.
    pub fn frequency(&self) -> Frequency {
        Frequency::new_from_raw(&self.0[1..])
    }
}

/// DLSettings represents LoRaWAN DLSettings.
#[derive(Debug, PartialEq)]
pub struct DLSettings(u8);

impl DLSettings {
    /// Constructs a new DLSettings from the provided data.
    pub fn new(byte: u8) -> DLSettings {
        DLSettings(byte)
    }

    /// The offset between the uplink data rate and the downlink data rate used to communicate with
    /// the end-device on the first reception slot (RX1).
    pub fn rx1_dr_offset(&self) -> u8 {
        self.0 >> 4 & 0x07
    }

    /// The data rate of a downlink using the second receive window.
    pub fn rx2_data_rate(&self) -> u8 {
        self.0 & 0x0f
    }

    /// The integer value of the DL Settings.
    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for DLSettings {
    fn from(v: u8) -> Self {
        DLSettings(v)
    }
}

/// Frequency represents a channel's central frequency.
#[derive(Debug, PartialEq)]
pub struct Frequency<'a>(&'a [u8]);

impl<'a> Frequency<'a> {
    /// Constructs a new Frequency from the provided bytes, without verifying if they are
    /// admissible.
    ///
    /// Improper use of this method could lead to panic during runtime!
    pub fn new_from_raw(bytes: &'a [u8]) -> Self {
        Frequency(bytes)
    }

    /// Constructs a new Frequency from the provided bytes.
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() != 3 {
            return None;
        }

        Some(Frequency(bytes))
    }

    /// Provides the decimal value in Hz of the frequency.
    pub fn value(&self) -> u32 {
        ((u32::from(self.0[2]) << 16) + (u32::from(self.0[1]) << 8) + u32::from(self.0[0])) * 100
    }
}

impl<'a> From<&'a [u8; 3]> for Frequency<'a> {
    fn from(v: &'a [u8; 3]) -> Self {
        Frequency(&v[..])
    }
}

impl<'a> AsRef<[u8]> for Frequency<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

/// RXParamSetupAnsPayload represents the RXParamSetupAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXParamSetupAnsPayload<'a>(&'a [u8; 1]);

impl<'a> RXParamSetupAnsPayload<'a> {
    /// Command identifier for RXParamSetupAnsPayload.
    create_type_const_fn!(cid, u8, 0x05);

    /// Whether RXParamSetupAnsPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, true);

    /// The len
    create_type_const_fn!(len, usize, 1);

    /// Check if the bytes can be used to create RXParamSetupAnsPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new RXParamSetupAnsPayload from the provided data.
    new_mac_cmd_helper!(RXParamSetupAns, RXParamSetupAnsPayload, 1);

    /// Whether the channel change was applied successsfully.
    create_ack_fn!(channel_ack, 0);

    /// Whether the rx2 data rate change was applied successsfully.
    create_ack_fn!(rx2_data_rate_ack, 1);

    /// Whether the rx1 data rate offset change was applied successsfully.
    create_ack_fn!(rx1_dr_offset_ack, 2);

    /// Whether the device has accepted the new parameters or not.
    pub fn ack(&self) -> bool {
        self.0[0] == 0x07
    }
}

/// DevStatusReqPayload represents the DevStatusReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DevStatusReqPayload();

impl DevStatusReqPayload {
    /// Command identifier for DevStatusReqPayload.
    create_type_const_fn!(cid, u8, 0x06);

    /// Whether DevStatusReqPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, false);

    /// The len
    create_type_const_fn!(len, usize, 0);

    /// Check if the bytes can be used to create DevStatusReqPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new DevStatusReqPayload from the provided data.
    new_mac_cmd_helper!(DevStatusReq, DevStatusReqPayload, 0);
}

/// DevStatusAnsPayload represents the DevStatusAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DevStatusAnsPayload<'a>(&'a [u8; 2]);

impl<'a> DevStatusAnsPayload<'a> {
    /// Command identifier for DevStatusAnsPayload.
    create_type_const_fn!(cid, u8, 0x06);

    /// Whether DevStatusAnsPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, true);

    /// The len
    create_type_const_fn!(len, usize, 2);

    /// Check if the bytes can be used to create DevStatusAnsPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new DevStatusAnsPayload from the provided data.
    new_mac_cmd_helper!(DevStatusAns, DevStatusAnsPayload, 2);

    /// The battery level of the device.
    ///
    /// Note: 0 means that the device is powered by an external source, 255 means that the device
    /// was unable to measure its battery level, any other value represents the actual battery
    /// level.
    create_value_reader_fn!(battery, 0);

    /// The margin is the demodulation signal-to-noise ratio in dB rounded to the nearest integer
    /// value for the last successfully received DevStatusReq command.
    pub fn margin(&self) -> i8 {
        ((self.0[1] << 2) as i8) >> 2
    }
}

/// NewChannelReqPayload represents the NewChannelReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct NewChannelReqPayload<'a>(&'a [u8; 5]);

impl<'a> NewChannelReqPayload<'a> {
    /// Command identifier for NewChannelReqPayload.
    create_type_const_fn!(cid, u8, 0x07);

    /// Whether NewChannelReqPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, false);

    /// The len
    create_type_const_fn!(len, usize, 5);

    /// Check if the bytes can be used to create NewChannelReqPayload.
    pub fn can_build_from(bytes: &[u8]) -> Result<(), String> {
        if bytes.len() < Self::len() {
            return Err(format_error(Self::len(), bytes.len()));
        }

        DataRateRange::can_build_from(bytes[4])
    }

    /// Constructs a new NewChannelReqPayload from the provided data.
    new_mac_cmd_helper!(NewChannelReq, NewChannelReqPayload, 5);

    /// The index of the channel being created or modified.
    create_value_reader_fn!(channel_index, 0);

    /// The frequency of the new or modified channel.
    pub fn frequency(&self) -> Frequency {
        Frequency::new_from_raw(&self.0[1..4])
    }

    /// The data rate range specifies allowed data rates for the new or modified channel.
    pub fn data_rate_range(&self) -> DataRateRange {
        DataRateRange::new_from_raw(self.0[4])
    }
}

/// DataRateRange represents LoRaWAN DataRateRange.
#[derive(Debug, PartialEq)]
pub struct DataRateRange(u8);

impl DataRateRange {
    /// Constructs a new DataRateRange from the provided byte, without checking for correctness.
    pub fn new_from_raw(byte: u8) -> DataRateRange {
        DataRateRange(byte)
    }

    /// Constructs a new DataRateRange from the provided byte.
    pub fn new(byte: u8) -> Result<DataRateRange, String> {
        if let Err(err) = Self::can_build_from(byte) {
            return Err(err);
        }

        Ok(Self::new_from_raw(byte))
    }

    /// Check if the byte can be used to create DataRateRange.
    pub fn can_build_from(byte: u8) -> Result<(), String> {
        if (byte >> 4) < (byte & 0x0f) {
            return Err(String::from(
                "data rate range can not have max data rate smaller than min data rate",
            ));
        }
        Ok(())
    }

    /// The highest data rate allowed on this channel.
    pub fn max_data_rate(&self) -> u8 {
        self.0 >> 4
    }

    /// The lowest data rate allowed on this channel.
    pub fn min_data_range(&self) -> u8 {
        self.0 & 0x0f
    }

    /// The integer value of the DataRateRange.
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
pub struct NewChannelAnsPayload<'a>(&'a [u8; 1]);

impl<'a> NewChannelAnsPayload<'a> {
    /// Command identifier for NewChannelAnsPayload.
    create_type_const_fn!(cid, u8, 0x07);

    /// Whether NewChannelAnsPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, true);

    /// The len
    create_type_const_fn!(len, usize, 1);

    /// Check if the bytes can be used to create NewChannelAnsPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new NewChannelAnsPayload from the provided data.
    new_mac_cmd_helper!(NewChannelAns, NewChannelAnsPayload, 1);

    /// Whether the channel frequency change was applied successsfully.
    create_ack_fn!(channel_freq_ack, 0);

    /// Whether the data rate range change was applied successsfully.
    create_ack_fn!(data_rate_range_ack, 1);

    /// Whether the device has accepted the new channel.
    pub fn ack(&self) -> bool {
        self.0[0] == 0x03
    }
}

/// RXTimingSetupReqPayload represents the RXTimingSetupReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXTimingSetupReqPayload<'a>(&'a [u8; 1]);

impl<'a> RXTimingSetupReqPayload<'a> {
    /// Command identifier for RXTimingSetupReqPayload.
    create_type_const_fn!(cid, u8, 0x08);

    /// Whether RXTimingSetupReqPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, false);

    /// The len
    create_type_const_fn!(len, usize, 1);

    /// Check if the bytes can be used to create RXTimingSetupReqPayload.
    create_type_const_fn!(can_build_from);

    /// Constructs a new RXTimingSetupReqPayload from the provided data.
    new_mac_cmd_helper!(RXTimingSetupReq, RXTimingSetupReqPayload, 1);

    /// Delay before the first RX window.
    pub fn delay(&self) -> u8 {
        self.0[0] & 0x0f
    }
}

/// RXTimingSetupAnsPayload represents the RXTimingSetupAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXTimingSetupAnsPayload();

impl RXTimingSetupAnsPayload {
    /// Command identifier for RXTimingSetupAnsPayload.
    create_type_const_fn!(cid, u8, 0x08);

    /// Whether RXTimingSetupAnsPayload is sent by the device or NS.
    create_type_const_fn!(uplink, bool, true);

    /// The len
    create_type_const_fn!(len, usize, 0);

    /// Check if the bytes can be used to create RXTimingSetupAnsPayload
    create_type_const_fn!(can_build_from);

    /// Constructs a new RXTimingSetupAnsPayload from the provided data.
    new_mac_cmd_helper!(RXTimingSetupAns, RXTimingSetupAnsPayload, 0);
}
