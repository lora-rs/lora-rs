use core::fmt::Debug;

pub use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};

/// Errors types reported during LoRa physical layer processing
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, defmt::Format, PartialEq)]
#[allow(dead_code, missing_docs)]
pub enum RadioError {
    SPI,
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
