use core::fmt::Debug;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, defmt::Format, PartialEq)]
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
    InvalidBaseAddress(usize, usize),
    PayloadSizeUnexpected(usize),
    PayloadSizeMismatch(usize, usize),
    InvalidSymbolTimeout,
    RetentionListExceeded,
    InvalidBandwidth,
    InvalidExplicitHeaderRequest,
    HeaderError,
    CRCErrorUnexpected,
    CRCErrorOnReceive,
    TransmitTimeout,
    ReceiveTimeout,
    TimeoutUnexpected,
    TransmitDoneUnexpected,
    ReceiveDoneUnexpected,
    DutyCycleUnsupported,
    DutyCycleRxContinuousUnsupported,
    CADUnexpected
}

pub struct RadioSystemError {
    pub rc_64khz_calibration: bool,
    pub rc_13mhz_calibration: bool,
    pub pll_calibration: bool,
    pub adc_calibration: bool,
    pub image_calibration: bool,
    pub xosc_start: bool,
    pub pll_lock: bool,
    pub pa_ramp: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PacketType {
    GFSK = 0x00,
    LoRa = 0x01,
    None = 0x0F,
}

impl PacketType {
    pub const fn value(self) -> u8 {
        self as u8
    }
    pub fn to_enum(value: u8) -> Self {
        if value == 0x00 {
            PacketType::GFSK
        } else if value == 0x01 {
            PacketType::LoRa
        } else {
            PacketType::None
        }
    }
}

#[derive(Clone, Copy)]
pub struct PacketStatus {
    pub rssi: i16,
    pub snr: i16,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RadioType {
    SX1261,
    SX1262,
    SX1276,
    SX1277,
    SX1278,
    SX1279
}

#[derive(Clone, Copy, PartialEq)]
pub enum RadioMode {
    Sleep,                    // sleep mode
    Standby,                  // standby mode
    FrequencySynthesis,       // frequency synthesis mode
    Transmit,                 // transmit mode
    Receive,                  // receive mode
    ReceiveDutyCycle,         // receive duty cycle mode
    ChannelActivityDetection, // channel activity detection mode
}

pub enum RadioState {
    Idle = 0x00,
    RxRunning = 0x01,
    TxRunning = 0x02,
    ChannelActivityDetecting = 0x03,
}

impl RadioState {
    /// Returns the value of the state.
    pub fn value(self) -> u8 {
        self as u8
    }
}

pub struct RadioStatus {
    pub cmd_status: u8,
    pub chip_mode: u8,
}

impl RadioStatus {
    pub fn value(self) -> u8 {
        (self.chip_mode << 4) | (self.cmd_status << 1)
    }
}

#[derive(Clone, Copy, PartialEq)]
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

#[derive(Clone, Copy, PartialEq)]
pub enum Bandwidth {
    _125KHz,
    _250KHz, 
    _500KHz,
}

impl Bandwidth {
    pub fn value_in_hz(self) -> u32 {
        match self {
            Bandwidth::_125KHz => 125000u32,
            Bandwidth::_250KHz => 250000u32,
            Bandwidth::_500KHz => 500000u32,
        }
    }
}

#[derive(Clone, Copy)]
pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

#[derive(Clone, Copy)]
pub struct ModulationParams {
    pub(crate) spreading_factor: SpreadingFactor,
    pub(crate) bandwidth: Bandwidth,
    pub(crate) coding_rate: CodingRate,
    pub(crate) low_data_rate_optimize: u8,
}

#[derive(Clone, Copy)]
pub struct PacketParams {
    pub(crate) preamble_length: u16, // number of LoRa symbols in the preamble
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

#[derive(Clone, Copy)]
pub struct DutyCycleParams {
    pub rx_time: u32,    // receive interval
    pub sleep_time: u32, // sleep interval
}
