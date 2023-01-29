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
    DIO1,
    DelayError,
    InvalidBaseAddress(usize, usize),
    PayloadSizeUnexpected(usize),
    PayloadSizeMismatch(usize, usize),
    RetentionListExceeded,
    InvalidBandwidth,
    HeaderError,
    CRCErrorUnexpected,
    CRCErrorOnReceive,
    TransmitTimeout,
    ReceiveTimeout,
    TimeoutUnexpected,
    TransmitDoneUnexpected,
    ReceiveDoneUnexpected,
    CADUnexpected,
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
    pub rssi: i8,
    pub snr: i8,
    pub signal_rssi: i8,
    pub freq_error: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RadioType {
    SX1261,
    SX1262,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RadioMode {
    Sleep,                     // sleep mode
    Standby,                   // standby mode
    FrequencySynthesis,        // frequency synthesis mode
    Transmit,                  // transmit mode
    Receive,                   // receive mode
    ReceiveDutyCycle,          // receive duty cycle mode
    ChannelActivityDetection,  // channel activity detection mode
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
    _5 = 0x05,
    _6 = 0x06,
    _7 = 0x07,
    _8 = 0x08,
    _9 = 0x09,
    _10 = 0x0A,
    _11 = 0x0B,
    _12 = 0x0C,
}

impl SpreadingFactor {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Bandwidth {
    _500KHz = 0x06,
    _250KHz = 0x05,
    _125KHz = 0x04,
}

impl Bandwidth {
    pub fn value(self) -> u8 {
        self as u8
    }

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
    _4_5 = 0x01,
    _4_6 = 0x02,
    _4_7 = 0x03,
    _4_8 = 0x04,
}

impl CodingRate {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
pub struct ModulationParams {
    pub (crate) spreading_factor: SpreadingFactor,
    pub (crate) bandwidth: Bandwidth,
    pub (crate) coding_rate: CodingRate,
    pub (crate) low_data_rate_optimize: u8,
}

#[derive(Clone, Copy)]
pub struct PacketParams {
    pub (crate) preamble_length: u16,  // number of LoRa symbols in the preamble
    pub (crate) implicit_header: bool, // if the header is explicit, it will be transmitted in the LoRa packet, but is not transmitted if the header is implicit (known fixed length)
    pub (crate) payload_length: u8,
    pub (crate) crc_on: bool,
    pub (crate) iq_inverted: bool,
}

#[derive(Clone, Copy)]
pub enum CADSymbols {
    _1 = 0x00,
    _2 = 0x01,
    _4 = 0x02,
    _8 = 0x03,
    _16 = 0x04,
}

impl CADSymbols {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
pub enum CADExitMode {
    CADOnly = 0x00,
    CADRx = 0x01,
    CADLBT = 0x10,
}

impl CADExitMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}
