//! LoRaWAN MAC layer command and payload handling support.
//!
//! A MAC command consists of a command identifier (CID) of 1 octet followed
//! by a possibly empty command-specific sequence of octets (payload).
use core::marker::PhantomData;
use lorawan_macros::CommandHandler;

#[deprecated(note = "Use lorawan::types::ChannelMask")]
#[doc(hidden)]
pub use crate::types::ChannelMask;

#[deprecated(note = "Use lorawan::types::DataRateRange")]
#[doc(hidden)]
use crate::types::DataRateRange;

#[deprecated(note = "Use lorawan::types::DLSettings")]
#[doc(hidden)]
pub use crate::types::DLSettings;

#[deprecated(note = "Use lorawan::types::Frequency")]
#[doc(hidden)]
pub use crate::types::Frequency;

#[deprecated(note = "Use lorawan::types::Redundancy")]
#[doc(hidden)]
pub use crate::types::Redundancy;

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error {
    UnknownMacCommand,
    BufferTooShort,
    InvalidIndex,
    InvalidDataRateRange,
    RFU,
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

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
/// Downlink MAC commands, transmitted by Network Server
pub enum DownlinkMacCommand<'a> {
    // LoRaWAN 1.0.0+ commands
    /// LinkCheckAns payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x02, len = 2)]
    LinkCheckAns(LinkCheckAnsPayload<'a>),

    /// LinkADRReq payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x03, len = 4)]
    LinkADRReq(LinkADRReqPayload<'a>),

    /// DutyCycleReq Payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x04, len = 1)]
    DutyCycleReq(DutyCycleReqPayload<'a>),

    /// RXParamSetupReq payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x05, len = 4)]
    RXParamSetupReq(RXParamSetupReqPayload<'a>),

    /// DevStatusReq payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x06, len = 0)]
    DevStatusReq(DevStatusReqPayload),

    /// NewChannelReq payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x07, len = 5)]
    NewChannelReq(NewChannelReqPayload<'a>),

    /// RXTimingSetupReq payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x08, len = 1)]
    RXTimingSetupReq(RXTimingSetupReqPayload<'a>),

    // LoRaWAN 1.0.2+ commands
    /// TXParamSetupReq payload handling (LoRaWAN 1.0.2+).
    #[cmd(cid = 0x09, len = 1)]
    TXParamSetupReq(TXParamSetupReqPayload<'a>),

    /// DlChannelReq payload (LoRaWAN 1.0.2+)
    #[cmd(cid = 0x0A, len = 4)]
    DlChannelReq(DlChannelReqPayload<'a>),

    // LoRaWAN 1.0.3+ commands
    /// DeviceTimeAns payload handling (LoRaWAN 1.0.3+)
    #[cmd(cid = 0x0D, len = 5)]
    DeviceTimeAns(DeviceTimeAnsPayload<'a>),
}

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
/// Uplink MAC commands, transmitted by End-device
pub enum UplinkMacCommand<'a> {
    // LoRaWAN 1.0.0 commands
    /// LinkCheckReq payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x02, len = 0)]
    LinkCheckReq(LinkCheckReqPayload),

    /// LinkADRAns payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x03, len = 1)]
    LinkADRAns(LinkADRAnsPayload<'a>),

    /// DutyCycleAns payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x04, len = 0)]
    DutyCycleAns(DutyCycleAnsPayload),

    /// RXParamSetupAns payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x05, len = 1)]
    RXParamSetupAns(RXParamSetupAnsPayload<'a>),

    /// DevStatusAns payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x06, len = 2)]
    DevStatusAns(DevStatusAnsPayload<'a>),

    /// NewChannelAns payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x07, len = 1)]
    NewChannelAns(NewChannelAnsPayload<'a>),

    /// RXTimingSetupAns payload handling (LoRaWAN 1.0.0+)
    #[cmd(cid = 0x08, len = 0)]
    RXTimingSetupAns(RXTimingSetupAnsPayload),

    // LoRaWAN 1.0.2+ commands
    /// TXParamSetupAns payload handling (LoRaWAN 1.0.2+)
    #[cmd(cid = 0x09, len = 0)]
    TXParamSetupAns(TXParamSetupAnsPayload),

    /// DlChannelAns payload handling (LoRaWAN 1.0.2+)
    #[cmd(cid = 0x0A, len = 1)]
    DlChannelAns(DlChannelAnsPayload<'a>),

    // 1.0.3+
    /// DeviceTimeReq payload handling (LoRaWAN 1.0.3+)
    #[cmd(cid = 0x0D, len = 0)]
    DeviceTimeReq(DeviceTimeReqPayload),
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
pub fn parse_uplink_mac_commands(data: &[u8]) -> MacCommandIterator<'_, UplinkMacCommand<'_>> {
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
pub fn parse_downlink_mac_commands(data: &[u8]) -> MacCommandIterator<'_, DownlinkMacCommand<'_>> {
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

impl LinkCheckAnsPayload<'_> {
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

impl LinkADRReqPayload<'_> {
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

impl LinkADRAnsPayload<'_> {
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

impl DutyCycleReqPayload<'_> {
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

impl RXParamSetupReqPayload<'_> {
    /// Downlink settings - namely rx1_dr_offset and rx2_data_rate.
    pub fn dl_settings(&self) -> DLSettings {
        DLSettings::new(self.0[0])
    }

    /// RX2 frequency.
    pub fn frequency(&self) -> Frequency<'_> {
        Frequency::new_from_raw(&self.0[1..])
    }
}

impl RXParamSetupAnsPayload<'_> {
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

impl DevStatusAnsPayload<'_> {
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

impl NewChannelReqPayload<'_> {
    create_value_reader_fn!(
        /// The index of the channel being created or modified.
        channel_index,
        0
    );

    /// The frequency of the new or modified channel.
    pub fn frequency(&self) -> Frequency<'_> {
        Frequency::new_from_raw(&self.0[1..4])
    }

    /// The data rate range specifies allowed data rates for the new or modified channel.
    pub fn data_rate_range(&self) -> Result<DataRateRange, Error> {
        DataRateRange::new(self.0[4])
    }
}

impl NewChannelAnsPayload<'_> {
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

impl RXTimingSetupReqPayload<'_> {
    /// Delay before the first RX window.
    pub fn delay(&self) -> u8 {
        self.0[0] & 0x0f
    }
}

impl TXParamSetupReqPayload<'_> {
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
    pub fn frequency(&self) -> Frequency<'_> {
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
