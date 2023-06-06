use core::fmt::Debug;

/// Errors types reported during LoRa physical layer processing
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, defmt::Format, PartialEq)]
#[allow(dead_code, missing_docs)]
pub enum RadioError {
    SPI,
    NSS,
    Reset,
    RfSwitchRx,
    RfSwitchTx,
    Busy,
    Irq,
    DIO1,
    DelayError,
    OpError(u8),
    InvalidBaseAddress(usize, usize),
    PayloadSizeUnexpected(usize),
    PayloadSizeMismatch(usize, usize),
    InvalidSymbolTimeout,
    RetentionListExceeded,
    UnavailableSpreadingFactor,
    UnavailableBandwidth,
    UnavailableCodingRate,
    InvalidBandwidthForFrequency,
    InvalidSF6ExplicitHeaderRequest,
    InvalidOutputPower,
    InvalidOutputPowerForFrequency,
    HeaderError,
    CRCErrorUnexpected,
    CRCErrorOnReceive,
    TransmitTimeout,
    ReceiveTimeout,
    PollingTimeout,
    TimeoutUnexpected,
    TransmitDoneUnexpected,
    ReceiveDoneUnexpected,
    DutyCycleUnsupported,
    DutyCycleRxContinuousUnsupported,
    CADUnexpected,
    RngUnsupported,
    BoardTypeUnsupportedForRadioKind,
}

/// Status for a received packet
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub struct PacketStatus {
    pub rssi: i16,
    pub snr: i16,
}

/// LoRa boards supported by this crate.
/// In addition, custom boards (possibly proprietary) can be supported by using the custom board and chip types and
/// external implementations of the RadioKind and (in some cases) InterfaceVariant traits.  For instance:
/// let iv = ExternalInterfaceVariantImpl::new(..params...)
/// LoRa::new(ExternalRadioKindImpl::new(BoardType::CustomSx1261_2, spi, iv), ...other_params...)
#[derive(Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum BoardType {
    CustomSx1261_2,
    CustomSx1276_7_8_9,
    GenericSx1261, // placeholder for Sx1261-specific features
    HeltecWifiLoraV31262,
    RpPicoWaveshareSx1262,
    Rak4631Sx1262,
    Rak3172Sx1262,
    Stm32l0Sx1276,
    Stm32wlSx1262,
}

/// LoRa chips supported by this crate
#[derive(Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum ChipType {
    CustomSx1261_2,
    CustomSx1276_7_8_9,
    Sx1261,
    Sx1262,
    Sx1276,
    Sx1277,
    Sx1278,
    Sx1279,
}

impl From<BoardType> for ChipType {
    fn from(board_type: BoardType) -> Self {
        match board_type {
            BoardType::CustomSx1261_2 => ChipType::CustomSx1261_2,
            BoardType::CustomSx1276_7_8_9 => ChipType::CustomSx1276_7_8_9,
            BoardType::GenericSx1261 => ChipType::Sx1261,
            BoardType::HeltecWifiLoraV31262 => ChipType::Sx1262,
            BoardType::RpPicoWaveshareSx1262 => ChipType::Sx1262,
            BoardType::Rak4631Sx1262 => ChipType::Sx1262,
            BoardType::Rak3172Sx1262 => ChipType::Sx1262,
            BoardType::Stm32l0Sx1276 => ChipType::Sx1276,
            BoardType::Stm32wlSx1262 => ChipType::Sx1262,
        }
    }
}

/// The state of the radio
#[derive(Clone, Copy, defmt::Format, PartialEq)]
#[allow(missing_docs)]
pub enum RadioMode {
    Sleep,                    // sleep mode
    Standby,                  // standby mode
    FrequencySynthesis,       // frequency synthesis mode
    Transmit,                 // transmit mode
    Receive,                  // receive mode
    ReceiveDutyCycle,         // receive duty cycle mode
    ChannelActivityDetection, // channel activity detection mode
}

/// Valid spreading factors for one or more LoRa chips supported by this crate
#[derive(Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum SpreadingFactor {
    _5,
    _6,
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
}

/// Valid bandwidths for one or more LoRa chips supported by this crate
#[derive(Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum Bandwidth {
    _7KHz,
    _10KHz,
    _15KHz,
    _20KHz,
    _31KHz,
    _41KHz,
    _62KHz,
    _125KHz,
    _250KHz,
    _500KHz,
}

impl Bandwidth {
    /// Convert to Hertz
    pub fn value_in_hz(self) -> u32 {
        match self {
            Bandwidth::_7KHz => 7810u32,
            Bandwidth::_10KHz => 10420u32,
            Bandwidth::_15KHz => 15630u32,
            Bandwidth::_20KHz => 20830u32,
            Bandwidth::_31KHz => 31250u32,
            Bandwidth::_41KHz => 41670u32,
            Bandwidth::_62KHz => 62500u32,
            Bandwidth::_125KHz => 125000u32,
            Bandwidth::_250KHz => 250000u32,
            Bandwidth::_500KHz => 500000u32,
        }
    }
}

/// Valid coding rates for one or more LoRa chips supported by this crate
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

/// Modulation parameters for a send and/or receive communication channel
pub struct ModulationParams {
    pub(crate) spreading_factor: SpreadingFactor,
    pub(crate) bandwidth: Bandwidth,
    pub(crate) coding_rate: CodingRate,
    pub(crate) low_data_rate_optimize: u8,
    pub(crate) frequency_in_hz: u32,
}

/// Packet parameters for a send or receive communication channel
pub struct PacketParams {
    pub(crate) preamble_length: u16,  // number of LoRa symbols in the preamble
    pub(crate) implicit_header: bool, // if the header is explicit, it will be transmitted in the LoRa packet, but is not transmitted if the header is implicit (known fixed length)
    pub(crate) payload_length: u8,
    pub(crate) crc_on: bool,
    pub(crate) iq_inverted: bool,
}

impl PacketParams {
    pub(crate) fn set_payload_length(&mut self, payload_length: usize) -> Result<(), RadioError> {
        if payload_length > 255 {
            return Err(RadioError::PayloadSizeUnexpected(payload_length));
        }
        self.payload_length = payload_length as u8;
        Ok(())
    }
}

/// Receive duty cycle parameters
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub struct DutyCycleParams {
    pub rx_time: u32,    // receive interval
    pub sleep_time: u32, // sleep interval
}
