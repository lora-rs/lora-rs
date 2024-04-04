pub use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};

/// Errors types reported during LoRa physical layer processing
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, defmt::Format, PartialEq)]
#[allow(missing_docs)]
pub enum RadioError {
    SPI,
    Reset,
    RfSwitchRx,
    RfSwitchTx,
    Busy,
    Irq,
    DIO1,
    InvalidConfiguration,
    InvalidRadioMode,
    OpError(u8),
    InvalidBaseAddress(usize, usize),
    PayloadSizeUnexpected(usize),
    PayloadSizeMismatch(usize, usize),
    UnavailableSpreadingFactor,
    UnavailableBandwidth,
    InvalidBandwidthForFrequency,
    InvalidSF6ExplicitHeaderRequest,
    InvalidOutputPowerForFrequency,
    TransmitTimeout,
    ReceiveTimeout,
    DutyCycleUnsupported,
    RngUnsupported,
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
pub enum RadioMode {
    /// Sleep mode
    Sleep,
    /// Standby mode
    Standby,
    /// Frequency synthesis mode
    FrequencySynthesis,
    /// Transmit (TX) mode
    Transmit,
    /// Receive (RX) mode
    Receive(RxMode),
    /// Channel activity detection (CAD) mode
    ChannelActivityDetection,
}

impl From<RxMode> for RadioMode {
    fn from(rx_mode: RxMode) -> Self {
        RadioMode::Receive(rx_mode)
    }
}

/// Listening mode for LoRaWAN packet detection/reception
#[derive(Clone, Copy, defmt::Format, PartialEq)]
pub enum RxMode {
    /// Single shot Rx Mode to listen until packet preamble is detected or RxTimeout occurs.
    /// The device will stay in RX Mode until a packet is received.
    /// Preamble length as symbols is configured via following registers:
    /// sx126x: uses `SetLoRaSymbNumTimeout(0 < n < 255)` + `SetStopRxTimerOnPreamble(1)`
    /// sx127x: uses `RegSymbTimeout (4 < n < 1023)`
    // TODO: Single mode with time-based timeout is available on sx126x, but not sx127x
    Single(u16),
    /// Continuous Rx mode to listen for incoming packets continuously
    Continuous,
    /// Receive in Duty Cycle mode (NB! Not supported on sx127x)
    DutyCycle(DutyCycleParams),
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
#[derive(Clone, Copy, defmt::Format, PartialEq)]
pub struct DutyCycleParams {
    /// receive interval
    pub rx_time: u32,
    /// sleep interval
    pub sleep_time: u32,
}
