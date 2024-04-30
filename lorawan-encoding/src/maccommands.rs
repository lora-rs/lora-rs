// Copyright (c) 2018,2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use core::marker::PhantomData;

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    UnknownMacCommand,
    BufferTooShort,
    InvalidIndex,
    InvalidDataRateRange,
}

pub trait SerializableMacCommand {
    fn payload_bytes(&self) -> &[u8];
    fn cid(&self) -> u8;
    fn payload_len(&self) -> usize;
}

/// Calculates length in bytes of a sequence of MAC commands, including CIDs.
pub fn mac_commands_len(cmds: &[&dyn SerializableMacCommand]) -> usize {
    cmds.iter().map(|mc| mc.payload_len() + 1).sum()
}

macro_rules! mac_cmd_zero_len {
    (

        $(
            $(#[$outer:meta])*
            struct $type:ident[cmd=$name:ident, cid=$cid:expr, uplink=$uplink:expr]
            )*
    ) => {
        $(
            $(#[$outer])*
            pub struct $type();

            impl $type {
                /// Creation.
                pub fn new(_: &[u8]) -> $type {
                    $type()
                }

                /// Duplicate fn to be compatible with the mac_cmds macro
                pub fn new_from_raw(_: &[u8]) ->$type {
                    $type()
                }

                /// Get the CID.
                pub const fn cid() -> u8 {
                    $cid
                }

                /// Sent by end device or sent by network server.
                pub const fn uplink() -> bool {
                    $uplink
                }

                /// Length of empty payload.
                pub const fn len() -> usize {
                    0
                }

                /// Reference to the empty payload.
                pub fn bytes (&self) -> &[u8]{
                    &[]
                }
            }
        )*
    }
}

macro_rules! mac_cmds {
    (

        $(
            $(#[$outer:meta])*
            struct $type:ident[cmd=$name:ident, cid=$cid:expr, uplink=$uplink:expr, size=$size:expr]
            )*
    ) => {
        $(
            $(#[$outer])*
            pub struct $type<'a>(pub(crate) &'a [u8]);

            impl<'a> $type<'a> {
                /// Creates a new instance of the MAC command if there is enought data.
                pub fn new(data: &'a [u8]) -> Result<$type<'a>, Error> {
                    if data.len() != $size {
                        Err(Error::BufferTooShort)
                    } else {
                        Ok($type(&data))
                    }
                }
                /// Constructs a new instance of the MAC command from the provided data,
                /// without verifying the data length.
                ///
                /// Improper use of this method could lead to panic during runtime!
                pub fn new_from_raw(data: &'a [u8]) ->$type<'a> {
                    $type(&data)
                }


                /// Get the CID.
                pub const fn cid() -> u8 {
                    $cid
                }

                /// Sent by end device or sent by network server.
                pub const fn uplink() -> bool {
                    $uplink
                }

                /// Length of payload without the CID.
                pub const fn len() -> usize {
                    $size
                }

                /// Reference to the payload.
                pub fn bytes (&self) -> &[u8]{
                    self.0
                }
            }
        )*
    }
}

macro_rules! mac_cmds_enum {
    (
        $outer_vis:vis enum $outer_type:ident$(<$outer_lifetime:lifetime>),* {
        $(
            $name:ident($type:ident$(<$lifetime:lifetime>),*)
        )*
    }
    ) => {
        #[derive(Debug, PartialEq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        #[allow(clippy::len_without_is_empty, missing_docs)]
        $outer_vis enum $outer_type$(<$outer_lifetime>)* {
            $(
                $name($type$(<$lifetime>)*),
            )*
        }
        impl$(<$outer_lifetime>)* $outer_type$(<$outer_lifetime>)* {
            /// Get the length.
            pub fn len(&self) -> usize {
                match *self {
                    $(
                        Self::$name(_) => $type::len(),
                    )*
                }
            }

            /// Sent by end device or sent by network server.
            pub fn uplink(&self) -> bool {
                match *self {
                    $(
                        Self::$name(_) => $type::uplink(),
                    )*
                }
            }

            /// Get reference to the data.
            pub fn bytes(&self) -> &[u8] {
                match *self {
                    $(
                        Self::$name(ref v) => v.bytes(),
                    )*
                }
            }
        }

        impl$(<$outer_lifetime>)* SerializableMacCommand for $outer_type$(<$outer_lifetime>)* {
            fn payload_bytes(&self) -> &[u8] {
                &self.bytes()
            }

            fn cid(&self) -> u8 {
                match *self {
                    $(
                        Self::$name(_) => $type::cid(),
                    )*
                }
            }

            fn payload_len(&self) -> usize {
                self.len()
            }
        }

        impl$(<$outer_lifetime>)* Iterator for MacCommandIterator<$($outer_lifetime)*, $outer_type$(<$outer_lifetime>)*> {
            type Item = $outer_type$(<$outer_lifetime>)*;

            fn next(&mut self) -> Option<Self::Item> {
                if self.index < self.data.len() {
                    let data = &self.data[self.index..];
                    $(
                        if data[0] == $type::cid() && data.len() >= $type::len() {
                            self.index = self.index + $type::len()+1;
                            Some($outer_type::$name($type::new_from_raw(&data[1.. 1 + $type::len()])))
                        } else
                    )* {
                        None
                    }
                }else{
                    None
                }
            }
        }
        impl<'a> From<&'a super::parser::FHDR<'a>>
            for MacCommandIterator<$($outer_lifetime)*, $outer_type$(<$outer_lifetime>)*>
        {
            fn from(fhdr: &'a super::parser::FHDR) -> Self {
                Self {
                    data: &fhdr.data(),
                    index: 0,
                    item: core::marker::PhantomData,
                }
            }
        }

        impl<'a> From<&'a super::parser::FRMMacCommands<'a>>
            for MacCommandIterator<$($outer_lifetime)*, $outer_type$(<$outer_lifetime>)*>
        {
            fn from(frmm: &'a super::parser::FRMMacCommands) -> Self {
                Self {
                    data: frmm.data(),
                    index: 0,
                    item: core::marker::PhantomData,
                }
            }
        }
    }
}
mac_cmds_enum! {
    pub enum DownlinkMacCommand<'a> {
        LinkCheckAns(LinkCheckAnsPayload<'a>)
        LinkADRReq(LinkADRReqPayload<'a>)
        DutyCycleReq(DutyCycleReqPayload<'a>)
        RXParamSetupReq(RXParamSetupReqPayload<'a>)
        DevStatusReq(DevStatusReqPayload)
        NewChannelReq(NewChannelReqPayload<'a>)
        RXTimingSetupReq(RXTimingSetupReqPayload<'a>)
        TXParamSetupReq(TXParamSetupReqPayload<'a>)
        DlChannelReq(DlChannelReqPayload<'a>)
        DeviceTimeAns(DeviceTimeAnsPayload<'a>)
    }
}

mac_cmds_enum! {
    pub enum UplinkMacCommand<'a> {
        LinkCheckReq(LinkCheckReqPayload)
        LinkADRAns(LinkADRAnsPayload<'a>)
        DutyCycleAns(DutyCycleAnsPayload)
        RXParamSetupAns(RXParamSetupAnsPayload<'a>)
        DevStatusAns(DevStatusAnsPayload<'a>)
        NewChannelAns(NewChannelAnsPayload<'a>)
        RXTimingSetupAns(RXTimingSetupAnsPayload)
        TXParamSetupAns(TXParamSetupAnsPayload)
        DlChannelAns(DlChannelAnsPayload<'a>)
        DeviceTimeReq(DeviceTimeReqPayload)
    }
}

mac_cmd_zero_len! {
    /// LinkCheckReqPayload represents the LinkCheckReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct LinkCheckReqPayload[cmd=LinkCheckReq, cid=0x02, uplink=true]

    /// DutyCycleAnsPayload represents the DutyCycleAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DutyCycleAnsPayload[cmd=DutyCycleAns, cid=0x04, uplink=true]

    /// DevStatusReqPayload represents the DevStatusReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DevStatusReqPayload[cmd=DevStatusReq, cid=0x06, uplink=false]

    /// RXTimingSetupAnsPayload represents the RXTimingSetupAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct RXTimingSetupAnsPayload[cmd=RXTimingSetupAns, cid=0x08, uplink=true]

    /// TXParamSetupAnsPayload represents the TXParamSetupAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct TXParamSetupAnsPayload[cmd=TXParamSetupAns, cid=0x09, uplink=true]

    /// DeviceTimeReqPayload represents the DeviceTimeReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DeviceTimeReqPayload[cmd=DeviceTimeReq, cid=0x0D, uplink=true]

}

mac_cmds! {
    /// LinkCheckAnsPayload represents the LinkCheckAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct LinkCheckAnsPayload[cmd=LinkCheckAns, cid=0x02, uplink=false, size=2]

    /// LinkADRReqPayload represents the LinkADRReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct LinkADRReqPayload[cmd=LinkADRReq, cid=0x03, uplink=false, size=4]

    /// LinkADRAnsPayload represents the LinkADRAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct LinkADRAnsPayload[cmd=LinkADRAns, cid=0x03, uplink=true, size=1]

    /// DutyCycleReqPayload represents the DutyCycleReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DutyCycleReqPayload[cmd=DutyCycleReq, cid=0x04, uplink=false, size=1]

    /// RXParamSetupReqPayload represents the RXParamSetupReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct RXParamSetupReqPayload[cmd=RXParamSetupReq, cid=0x05, uplink=false, size=4]

    /// RXParamSetupAnsPayload represents the RXParamSetupAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct RXParamSetupAnsPayload[cmd=RXParamSetupAns, cid=0x05, uplink=true, size=1]

    /// DevStatusAnsPayload represents the DevStatusAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DevStatusAnsPayload[cmd=DevStatusAns, cid=0x06, uplink=false, size=2]

    /// NewChannelReqPayload represents the NewChannelReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct NewChannelReqPayload[cmd=NewChannelReq, cid=0x07, uplink=false, size=5]

    /// NewChannelAnsPayload represents the NewChannelAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct NewChannelAnsPayload[cmd=NewChannelAns, cid=0x07, uplink=true, size=1]

    /// RXTimingSetupReqPayload represents the RXTimingSetupReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct RXTimingSetupReqPayload[cmd=RXTimingSetupReq, cid=0x08, uplink=false, size=1]

    /// TXParamSetupReqPayload represents the TXParamSetupReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct TXParamSetupReqPayload[cmd=TXParamSetupReq, cid=0x09, uplink=false, size=1]

    /// DlChannelReqPayload represents the DlChannelReq LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DlChannelReqPayload[cmd=DlChannelReq, cid=0x0A, uplink=false, size=4]

    /// DlChannelAnsPayload represents the DlChannelAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DlChannelAnsPayload[cmd=DlChannelAns, cid=0x0A, uplink=true, size=1]

    /// DeviceTimeAnsPayload represents the DeviceTimeAns LoRaWAN MACCommand.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[derive(Debug, PartialEq, Eq)]
    struct DeviceTimeAnsPayload[cmd=DeviceTimeAns, cid=0x0D, uplink=false, size=5]
}

macro_rules! create_ack_fn {
    (
        $(#[$outer:meta])*
        $fn_name:ident, $offset:expr
    ) => (
        $(#[$outer])*
        pub fn $fn_name(&self) -> bool {
            self.0[0] & (0x01 << $offset) != 0
        }
    )
}

macro_rules! create_value_reader_fn {
    (
        $(#[$outer:meta])*
        $fn_name:ident, $index:expr
    ) => (
        $(#[$outer])*
        pub fn $fn_name(&self) -> u8 {
            self.0[$index]
        }
    )
}

/// Parses bytes to uplink MAC commands if possible.
///
/// Could return error if some values are out of range or the payload does not end at MAC command
/// boundry.
/// # Argument
///
/// * bytes - the data from which the MAC commands are to be built.
///
/// # Examples
///
/// ```
/// let mut data = vec![0x02, 0x03, 0x00];
/// let mac_cmds: Vec<lorawan::maccommands::UplinkMacCommand> =
///     lorawan::maccommands::parse_uplink_mac_commands(&data).collect();
/// ```
pub fn parse_uplink_mac_commands(data: &[u8]) -> MacCommandIterator<UplinkMacCommand> {
    MacCommandIterator::new(data)
}
/// Parses bytes to downlink MAC commands if possible.
///
/// Could return error if some values are out of range or the payload does not end at MAC command
/// boundry.
/// # Argument
///
/// * bytes - the data from which the MAC commands are to be built.
///
/// # Examples
///
/// ```
/// let mut data = vec![0x02, 0x03, 0x00];
/// let mac_cmds: Vec<lorawan::maccommands::DownlinkMacCommand> =
///     lorawan::maccommands::parse_downlink_mac_commands(&data).collect();
/// ```
pub fn parse_downlink_mac_commands(data: &[u8]) -> MacCommandIterator<DownlinkMacCommand> {
    MacCommandIterator::new(data)
}

/// Implementation of iterator for MAC commands.
pub struct MacCommandIterator<'a, T> {
    pub(crate) data: &'a [u8],
    pub(crate) index: usize,
    pub(crate) item: PhantomData<T>,
}

impl<'a, T> MacCommandIterator<'a, T> {
    /// Creation.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, index: 0, item: Default::default() }
    }
}

// impl<'a, T> Iterator for MacCommandIterator<'a, T> {
//     type Item = T;

//     fn next(&mut self) -> Option<Self::Item> {
//         if self.index < self.data.len() {
//             if let Ok((l, v)) = parse_one_mac_cmd(&self.data[self.index..], self.uplink) {
//                 self.index += 1 + l;
//                 return Some(v);
//             }
//         }
//         None
//     }
// }

impl<'a> LinkCheckAnsPayload<'a> {
    create_value_reader_fn!(
        /// The link margin in dB of the last successfully received LinkCheckReq command.
        margin,
        0
    );

    create_value_reader_fn!(
        /// The number of gateways that successfully received the last LinkCheckReq command.
        gateway_count,
        1
    );
}

impl<'a> From<&'a [u8; 2]> for LinkCheckAnsPayload<'a> {
    fn from(v: &'a [u8; 2]) -> Self {
        LinkCheckAnsPayload(v)
    }
}

impl<'a> LinkADRReqPayload<'a> {
    /// Data Rate that the device should use for its next transmissions.
    pub fn data_rate(&self) -> u8 {
        self.0[0] >> 4
    }

    /// TX Power that the device should use for its next transmissions.
    pub fn tx_power(&self) -> u8 {
        self.0[0] & 0x0f
    }

    /// Usable channels for next transmissions.
    pub fn channel_mask(&self) -> ChannelMask<2> {
        ChannelMask::<2>::new_from_raw(&self.0[1..3])
    }

    /// Provides information how channel mask is to be interpreted and how many times each message
    /// should be repeated.
    pub fn redundancy(&self) -> Redundancy {
        Redundancy::new(self.0[3])
    }
}

impl<'a> From<&'a [u8; 4]> for LinkADRReqPayload<'a> {
    fn from(v: &'a [u8; 4]) -> Self {
        LinkADRReqPayload(v)
    }
}

/// ChannelMask represents the ChannelMask from LoRaWAN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMask<const N: usize>([u8; N]);

impl<const N: usize> Default for ChannelMask<N> {
    fn default() -> Self {
        ChannelMask([0xFF; N])
    }
}

#[cfg(feature = "serde")]
impl<const N: usize> serde::Serialize for ChannelMask<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for e in &self.0 {
            seq.serialize_element(e)?;
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
struct ChannelMaskDeserializer<const N: usize>;

#[cfg(feature = "serde")]
impl<'de, const N: usize> serde::de::Visitor<'de> for ChannelMaskDeserializer<N> {
    type Value = ChannelMask<N>;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter.write_str("ChannelMask byte.")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut arr = [0; N];
        let mut index = 0;
        while let Some(el) = seq.next_element()? {
            if index >= N {
                return Err(serde::de::Error::custom("ChannelMask has too many elements"));
            } else {
                arr[index] = el;
                index += 1;
            }
        }
        Ok(ChannelMask(arr))
    }
}

#[cfg(feature = "serde")]
impl<'de, const N: usize> serde::Deserialize<'de> for ChannelMask<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(ChannelMaskDeserializer {})
    }
}

impl<const N: usize> ChannelMask<N> {
    /// Constructs a new ChannelMask from the provided data.
    pub fn new(data: &[u8]) -> Result<Self, Error> {
        if data.len() < N {
            return Err(Error::BufferTooShort);
        }
        Ok(Self::new_from_raw(data))
    }

    pub fn set_bank(&mut self, index: usize, value: u8) {
        self.0[index] = value;
    }

    /// Enable or disable a specific channel. Recall that LoRaWAN channel numbers start indexing
    /// at zero.
    ///
    /// Improper use of this method could lead to out of bounds panic during runtime!
    pub fn set_channel(&mut self, channel: usize, set: bool) {
        let index = channel >> 3;
        let mut flag = 0b1 << (channel & 0x07);
        if set {
            self.0[index] |= flag;
        } else {
            flag = !flag;
            self.0[index] &= flag;
        }
    }

    pub fn get_index(&self, index: usize) -> u8 {
        self.0[index]
    }

    /// Constructs a new ChannelMask from the provided data, without verifying if they are
    /// admissible.
    ///
    /// Improper use of this method could lead to panic during runtime!
    pub fn new_from_raw(data: &[u8]) -> Self {
        let mut payload = [0; N];
        payload[..N].copy_from_slice(&data[..N]);
        ChannelMask(payload)
    }

    fn channel_enabled(&self, index: usize) -> bool {
        self.0[index >> 3] & (1 << (index & 0x07)) != 0
    }

    /// Verifies if a given channel is enabled.
    pub fn is_enabled(&self, index: usize) -> Result<bool, Error> {
        let index_limit = N * 8 - 1;
        if index > index_limit {
            return Err(Error::InvalidIndex);
        }
        Ok(self.channel_enabled(index))
    }

    /// Provides information for each of the 16 channels if they are enabled.
    pub fn statuses<const M: usize>(&self) -> [bool; M] {
        let mut res = [false; M];
        for (i, c) in res.iter_mut().enumerate() {
            *c = self.channel_enabled(i);
        }
        res
    }
}

impl<const N: usize> From<[u8; N]> for ChannelMask<N> {
    fn from(v: [u8; N]) -> Self {
        ChannelMask(v)
    }
}

impl<const N: usize> AsRef<[u8]> for ChannelMask<N> {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

/// Redundancy represents the LinkADRReq Redundancy from LoRaWAN.
#[derive(Debug, PartialEq, Eq)]
pub struct Redundancy(u8);

impl Redundancy {
    /// Constructs a new Redundancy from the provided data.
    pub fn new(data: u8) -> Self {
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

impl<'a> LinkADRAnsPayload<'a> {
    create_ack_fn!(
        /// Whether the channel mask change was applied successsfully.
        channel_mask_ack,
        0
    );

    create_ack_fn!(
        /// Whether the data rate change was applied successsfully.
        data_rate_ack,
        1
    );

    create_ack_fn!(
        /// Whether the power change was applied successsfully.
        powert_ack,
        2
    );

    /// Whether the device has accepted the new parameters or not.
    pub fn ack(&self) -> bool {
        self.0[0] == 0x07
    }
}

impl<'a> DutyCycleReqPayload<'a> {
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

impl<'a> RXParamSetupReqPayload<'a> {
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
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
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
        self.0
    }
}

impl<'a> RXParamSetupAnsPayload<'a> {
    create_ack_fn!(
        /// Whether the channel change was applied successsfully.
        channel_ack,
        0
    );

    create_ack_fn!(
        /// Whether the rx2 data rate change was applied successsfully.
        rx2_data_rate_ack,
        1
    );

    create_ack_fn!(
        /// Whether the rx1 data rate offset change was applied successsfully.
        rx1_dr_offset_ack,
        2
    );

    /// Whether the device has accepted the new parameters or not.
    pub fn ack(&self) -> bool {
        self.0[0] == 0x07
    }
}

impl<'a> DevStatusAnsPayload<'a> {
    create_value_reader_fn!(
        /// The battery level of the device.
        ///
        /// Note: 0 means that the device is powered by an external source, 255 means that the
        /// device was unable to measure its battery level, any other value represents the
        /// actual battery level.
        battery,
        0
    );

    /// The margin is the demodulation signal-to-noise ratio in dB rounded to the nearest integer
    /// value for the last successfully received DevStatusReq command.
    pub fn margin(&self) -> i8 {
        ((self.0[1] << 2) as i8) >> 2
    }
}

impl<'a> NewChannelReqPayload<'a> {
    create_value_reader_fn!(
        /// The index of the channel being created or modified.
        channel_index,
        0
    );

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
#[derive(Debug, PartialEq, Eq)]
pub struct DataRateRange(u8);

impl DataRateRange {
    /// Constructs a new DataRateRange from the provided byte, without checking for correctness.
    pub fn new_from_raw(byte: u8) -> DataRateRange {
        DataRateRange(byte)
    }

    /// Constructs a new DataRateRange from the provided byte.
    pub fn new(byte: u8) -> Result<DataRateRange, Error> {
        Self::can_build_from(byte)?;

        Ok(Self::new_from_raw(byte))
    }

    /// Check if the byte can be used to create DataRateRange.
    pub fn can_build_from(byte: u8) -> Result<(), Error> {
        if (byte >> 4) < (byte & 0x0f) {
            return Err(Error::InvalidDataRateRange);
        }
        Ok(())
    }

    /// The highest data rate allowed on this channel.
    pub fn max_data_rate(&self) -> u8 {
        self.0 >> 4
    }

    /// The lowest data rate allowed on this channel.
    pub fn min_data_rate(&self) -> u8 {
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

impl<'a> NewChannelAnsPayload<'a> {
    create_ack_fn!(
        /// Whether the channel frequency change was applied successsfully.
        channel_freq_ack,
        0
    );

    create_ack_fn!(
        /// Whether the data rate range change was applied successsfully.
        data_rate_range_ack,
        1
    );

    /// Whether the device has accepted the new channel.
    pub fn ack(&self) -> bool {
        self.0[0] == 0x03
    }
}

impl<'a> RXTimingSetupReqPayload<'a> {
    /// Delay before the first RX window.
    pub fn delay(&self) -> u8 {
        self.0[0] & 0x0f
    }
}

impl<'a> TXParamSetupReqPayload<'a> {
    pub fn downlink_dwell_time(&self) -> bool {
        self.0[0] & (1 << 5) != 0
    }
    pub fn uplink_dwell_time(&self) -> bool {
        self.0[0] & (1 << 4) != 0
    }
    pub fn max_eirp(&self) -> u8 {
        match self.0[0] & (0b1111) {
            0 => 8,
            1 => 10,
            2 => 12,
            3 => 13,
            4 => 14,
            5 => 16,
            6 => 18,
            7 => 20,
            8 => 21,
            9 => 24,
            10 => 26,
            11 => 27,
            12 => 29,
            13 => 30,
            14 => 33,
            15 => 36,
            _ => unreachable!(),
        }
    }
}

impl DlChannelReqPayload<'_> {
    create_value_reader_fn!(
        /// The index of the channel being created or modified.
        channel_index,
        0
    );

    /// The frequency of the new or modified channel.
    pub fn frequency(&self) -> Frequency {
        Frequency::new_from_raw(&self.0[1..4])
    }
}

impl DlChannelAnsPayload<'_> {
    create_ack_fn!(
        /// Channel frequency ok
        channel_freq_ack,
        0
    );

    create_ack_fn!(
        /// Uplink frequency exists
        uplink_freq_ack,
        1
    );

    /// Whether the device has accepted the new downlink frequency.
    pub fn ack(&self) -> bool {
        self.0[0] & 0x03 == 0x03
    }
}

impl DeviceTimeAnsPayload<'_> {
    pub fn seconds(&self) -> u32 {
        u32::from_le_bytes([self.0[3], self.0[2], self.0[1], self.0[0]])
    }
    //raw value in 1/256 seconds
    pub fn nano_seconds(&self) -> u32 {
        (self.0[4] as u32) * 3906250
    }
}
