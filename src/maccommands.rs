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
    format!(
        "not enough bytes to read: needs {}, given {}",
        expected, actual
    )
}

macro_rules! new_mac_cmd_helper  {
    ( $name:ident, $type:ident, 0 ) => {{
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
    pub fn cid() -> u8 {
        0x02
    }

    /// Constructs a new LinkCheckReqPayload.
    pub fn new<'a>(_: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(LinkCheckReq, LinkCheckReqPayload, 0)
    }

    /// Whether LinkCheckReqPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        true
    }
}

/// LinkCheckAnsPayload represents the LinkCheckAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkCheckAnsPayload<'a>(&'a [u8; 2]);

impl<'a> LinkCheckAnsPayload<'a> {
    /// Command identifier for LinkCheckAnsPayload.
    pub fn cid() -> u8 {
        0x02
    }

    /// Constructs a new LinkCheckAnsPayload from the provided data.
    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(LinkCheckAns, LinkCheckAnsPayload, data, 2)
    }

    /// Whether LinkCheckAnsPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        false
    }

    /// The link margin in dB of the last successfully received LinkCheckReq command.
    pub fn margin(&self) -> u8 {
        self.0[0]
    }

    /// The number of gateways that successfully received the last LinkCheckReq command.
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
    /// Command identifier for LinkADRReqPayload.
    pub fn cid() -> u8 {
        0x03
    }

    /// Whether LinkADRReqPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        false
    }

    /// Constructs a new LinkADRReqPayload from the provided data.
    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(LinkADRReq, LinkADRReqPayload, data, 4)
    }

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
pub struct ChannelMask<'a>(&'a [u8; 2]);

impl<'a> ChannelMask<'a> {
    /// Constructs a new ChannelMask from the provided data.
    pub fn new<'b>(data: &'b [u8]) -> Result<ChannelMask<'b>, String> {
        if data.len() < 2 {
            let msg = format!(
                "not enough bytes to read: needs {}, given {}",
                2,
                data.len(),
            );
            return Err(msg);
        }
        Ok(Self::new_from_raw(data))
    }

    /// Constructs a new ChannelMask from the provided data, without verifying if they are
    /// admissible.
    ///
    /// Improper use of this method could lead to panic during runtime!
    pub fn new_from_raw<'b>(data: &'b [u8]) -> ChannelMask<'b> {
        let payload = array_ref![&data[..2], 0, 2];
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

/// Redundancy represents the ChannelMask from LoRaWAN.
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
}

/// LinkADRAnsPayload represents the LinkADRAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkADRAnsPayload(u8);

impl LinkADRAnsPayload {
    /// Command identifier for LinkADRAnsPayload.
    pub fn cid() -> u8 {
        0x03
    }

    /// Whether LinkADRAnsPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        true
    }

    /// Constructs a new LinkADRAnsPayload from the provided data.
    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(LinkADRAns, LinkADRAnsPayload, data, 1)
    }

    /// Whether the channel mask change was applied successsfully.
    pub fn channel_mask_ack(&self) -> bool {
        self.0 & 0x01 != 0
    }

    /// Whether the data rate change was applied successsfully.
    pub fn data_rate_ack(&self) -> bool {
        self.0 & 0x02 != 0
    }

    /// Whether the power change was applied successsfully.
    pub fn powert_ack(&self) -> bool {
        self.0 & 0x04 != 0
    }

    /// Whether the device has accepted the new parameters or not.
    pub fn ack(&self) -> bool {
        self.0 == 0x07
    }
}

/// DutyCycleReqPayload represents the DutyCycleReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DutyCycleReqPayload(u8);

impl DutyCycleReqPayload {
    /// Command identifier for DutyCycleReqPayload.
    pub fn cid() -> u8 {
        0x04
    }

    /// Whether DutyCycleReqPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        false
    }

    /// Constructs a new DutyCycleReqPayload from the provided data.
    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(DutyCycleReq, DutyCycleReqPayload, data, 1)
    }

    /// Integer value of the max duty cycle field.
    pub fn max_duty_cycle_raw(&self) -> u8 {
        self.0 & 0x0f
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
    pub fn cid() -> u8 {
        0x04
    }

    /// Whether DutyCycleAnsPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        true
    }

    /// Constructs a new DutyCycleAnsPayload from the provided data.
    pub fn new<'b>(_: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(DutyCycleAns, DutyCycleAnsPayload, 0)
    }
}

/// RXParamSetupReqPayload represents the RXParamSetupReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXParamSetupReqPayload<'a>(&'a [u8; 4]);

impl<'a> RXParamSetupReqPayload<'a> {
    /// Command identifier for RXParamSetupReqPayload.
    pub fn cid() -> u8 {
        0x05
    }

    /// Whether RXParamSetupReqPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        false
    }

    /// Constructs a new RXParamSetupReqPayload from the provided data.
    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(RXParamSetupReq, RXParamSetupReqPayload, data, 4)
    }

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
    pub fn new_from_raw(bytes: &'a [u8]) -> Frequency {
        Frequency(bytes)
    }

    /// Constructs a new Frequency from the provided bytes.
    pub fn new(bytes: &'a [u8]) -> Option<Frequency> {
        if bytes.len() != 3 {
            return None;
        }

        Some(Frequency(bytes))
    }

    /// Provides the decimal value in Hz of the frequency.
    pub fn value(&self) -> u32 {
        (((self.0[2] as u32) << 16) + ((self.0[1] as u32) << 8) + (self.0[0] as u32)) * 100
    }
}

/// RXParamSetupAnsPayload represents the RXParamSetupAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXParamSetupAnsPayload(u8);

impl RXParamSetupAnsPayload {
    /// Command identifier for RXParamSetupAnsPayload.
    pub fn cid() -> u8 {
        0x05
    }

    /// Whether RXParamSetupAnsPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        true
    }

    /// Constructs a new RXParamSetupAnsPayload from the provided data.
    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(RXParamSetupAns, RXParamSetupAnsPayload, data, 1)
    }

    /// Whether the channel change was applied successsfully.
    pub fn channel_ack(&self) -> bool {
        (self.0 & 0x01) != 0
    }

    /// Whether the rx2 data rate change was applied successsfully.
    pub fn rx2_data_rate_ack(&self) -> bool {
        (self.0 & 0x02) != 0
    }

    /// Whether the rx1 data rate offset change was applied successsfully.
    pub fn rx1_dr_offset_ack(&self) -> bool {
        (self.0 & 0x04) != 0
    }

    /// Whether the device has accepted the new parameters or not.
    pub fn ack(&self) -> bool {
        self.0 == 0x07
    }
}

/// DevStatusReqPayload represents the DevStatusReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DevStatusReqPayload();

impl DevStatusReqPayload {
    /// Command identifier for DevStatusReqPayload.
    pub fn cid() -> u8 {
        0x06
    }

    /// Whether DevStatusReqPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        false
    }

    /// Constructs a new DevStatusReqPayload from the provided data.
    pub fn new<'b>(_: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(DevStatusReq, DevStatusReqPayload, 0)
    }
}

/// DevStatusAnsPayload represents the DevStatusAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct DevStatusAnsPayload<'a>(&'a [u8; 2]);

impl<'a> DevStatusAnsPayload<'a> {
    /// Command identifier for DevStatusAnsPayload.
    pub fn cid() -> u8 {
        0x06
    }

    /// Whether DevStatusAnsPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        true
    }

    /// Constructs a new DevStatusAnsPayload from the provided data.
    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(DevStatusAns, DevStatusAnsPayload, data, 2)
    }

    /// The battery level of the device.
    ///
    /// Note: 0 means that the device is powered by an external source, 255 means that the device
    /// was unable to measure its battery level, any other value represents the actual battery
    /// level.
    pub fn battery(&self) -> u8 {
        self.0[0]
    }

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
    pub fn cid() -> u8 {
        0x07
    }

    /// Whether NewChannelReqPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        false
    }

    /// Constructs a new NewChannelReqPayload from the provided data.
    pub fn new<'b>(data: &'b [u8]) -> Result<(MacCommand<'b>, usize), String> {
        new_mac_cmd_helper!(NewChannelReq, NewChannelReqPayload, data, 5)
    }

    /// The index of the channel being created or modified.
    pub fn channel_index(&self) -> u8 {
        self.0[0]
    }

    /// The frequency of the new or modified channel.
    pub fn frequency(&self) -> Frequency {
        Frequency::new_from_raw(&self.0[1..4])
    }

    /// The data rate range specifies allowed data rates for the new or modified channel.
    pub fn data_rate_range(&self) -> DataRateRange {
        DataRateRange::new(self.0[4])
    }
}

/// DataRateRange represents LoRaWAN DataRateRange.
#[derive(Debug, PartialEq)]
pub struct DataRateRange(u8);

impl DataRateRange {
    /// Constructs a new DataRateRange from the provided byte.
    pub fn new(byte: u8) -> DataRateRange {
        DataRateRange(byte)
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
pub struct NewChannelAnsPayload(u8);

impl NewChannelAnsPayload {
    /// Command identifier for NewChannelAnsPayload.
    pub fn cid() -> u8 {
        0x07
    }

    /// Whether NewChannelAnsPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        true
    }

    /// Constructs a new NewChannelAnsPayload from the provided data.
    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(NewChannelAns, NewChannelAnsPayload, data, 1)
    }

    /// Whether the data rate range change was applied successsfully.
    pub fn data_rate_range_ack(&self) -> bool {
        self.0 & 0x02 != 0
    }

    /// Whether the channel frequency change was applied successsfully.
    pub fn channel_freq_ack(&self) -> bool {
        self.0 & 0x01 != 0
    }

    /// Whether the device has accepted the new channel.
    pub fn ack(&self) -> bool {
        self.0 == 0x03
    }
}

/// RXTimingSetupReqPayload represents the RXTimingSetupReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXTimingSetupReqPayload(u8);

impl RXTimingSetupReqPayload {
    /// Command identifier for RXTimingSetupReqPayload.
    pub fn cid() -> u8 {
        0x08
    }

    /// Whether RXTimingSetupReqPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        false
    }

    /// Constructs a new RXTimingSetupReqPayload from the provided data.
    pub fn new<'a>(data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(RXTimingSetupReq, RXTimingSetupReqPayload, data, 1)
    }

    /// Delay before the first RX window.
    pub fn delay(&self) -> u8 {
        self.0 & 0x0f
    }
}

/// RXTimingSetupAnsPayload represents the RXTimingSetupAns LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct RXTimingSetupAnsPayload();

impl RXTimingSetupAnsPayload {
    /// Command identifier for RXTimingSetupAnsPayload.
    pub fn cid() -> u8 {
        0x08
    }

    /// Whether RXTimingSetupAnsPayload is sent by the device or NS.
    pub fn uplink() -> bool {
        true
    }

    /// Constructs a new RXTimingSetupAnsPayload from the provided data.
    pub fn new<'a>(_data: &'a [u8]) -> Result<(MacCommand<'a>, usize), String> {
        new_mac_cmd_helper!(RXTimingSetupAns, RXTimingSetupAnsPayload, 0)
    }
}
