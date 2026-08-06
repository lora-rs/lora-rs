use lora_modulation::BaseBandModulationParams;
pub use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};

/// Errors types reported during LoRa physical layer processing
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
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
    InvalidSyncWord,
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
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[allow(missing_docs)]
pub struct PacketStatus {
    pub rssi: i16,
    pub snr: i16,
}

/// The state of the radio
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
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
    /// Listen mode
    Listen,
    /// Channel activity detection (CAD) mode
    ChannelActivityDetection,
}

impl From<RxMode> for RadioMode {
    fn from(rx_mode: RxMode) -> Self {
        RadioMode::Receive(rx_mode)
    }
}

/// Listening mode for LoRaWAN packet detection/reception
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum RxMode {
    /// Single shot Rx Mode to listen until packet preamble is detected or RxTimeout occurs.
    /// The device will stay in RX Mode until a packet is received.
    /// Preamble length as symbols is configured via following registers:
    /// * sx126x: uses `SetLoRaSymbNumTimeout(0 < n < 255)` + `SetStopRxTimerOnPreamble(1)`
    /// * sx127x: uses `RegSymbTimeout (4 < n < 1023)`
    // TODO: Single mode with time-based timeout is available on sx126x, but not sx127x
    Single(u16),
    /// Continuous Rx mode to listen for incoming packets continuously
    Continuous,
    /// Receive in Duty Cycle mode (NB! Not supported on sx127x)
    DutyCycle(DutyCycleParams),
}

/// Modulation parameters for a send and/or receive communication channel
pub struct ModulationParams {
    /// Spreading Factor: higher value improves sensitivity at the cost of time-on-air
    pub spreading_factor: SpreadingFactor,
    /// Signal Bandwidth: lower value improves sensitivity at the cost of time-on-air
    pub bandwidth: Bandwidth,
    /// Coding Rate: controls number of redundancy bits
    pub coding_rate: CodingRate,
    /// Set to 1 to improve reliability at the cost of time-on-air.
    /// LoRaWAN enable this mode for SF11/12 at bandwidth 125kHz and SF12 at bandwidth 250kHz
    pub low_data_rate_optimize: u8,
    /// Channel frequency in Hertz
    pub frequency_in_hz: u32,
}

impl From<ModulationParams> for BaseBandModulationParams {
    fn from(value: ModulationParams) -> Self {
        Self::new(value.spreading_factor, value.bandwidth, value.coding_rate)
    }
}

/// Packet parameters for a send or receive communication channel
pub struct PacketParams {
    /// Number of LoRa symbols in the preamble (typical value are 12 for SF5/6 and 8 for SF7 to 12)
    pub preamble_length: u16,
    /// When true length, CodingRate and CRC must be known by the RX.
    /// When false, a header is automatically added to the packet allowing the RX to automatically discover the settings
    pub implicit_header: bool,
    /// Legth of the payload in number of bytes
    pub payload_length: u8,
    /// Enable CRC
    pub crc_on: bool,
    /// Use inverted chirp direction (generally used to distinguished uplink/downlink)
    pub iq_inverted: bool,
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
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct DutyCycleParams {
    /// receive interval
    pub rx_time: u32,
    /// sleep interval
    pub sleep_time: u32,
}

// The canonical sync word form is the 16-bit value the sx126x writes to its
// two sync word registers. The single-byte form used by the older chips
// (sx127x register, lr11xx SetLoRaSyncWord) maps into it as 0xYZ <-> 0xY4Z4;
// the LoRaWAN public/private words 0x34/0x12 are 0x3444/0x1424.
pub(crate) fn sync_word_from_legacy(sync_word: u8) -> u16 {
    u16::from_be_bytes([(sync_word & 0xF0) | 0x04, ((sync_word & 0x0F) << 4) | 0x04])
}

pub(crate) fn sync_word_to_legacy(sync_word: u16) -> Result<u8, RadioError> {
    let [msb, lsb] = sync_word.to_be_bytes();
    if (msb & 0x0F == 0x04) && (lsb & 0x0F == 0x04) {
        Ok((msb & 0xF0) | (lsb >> 4))
    } else {
        Err(RadioError::InvalidSyncWord)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_word_legacy_mapping() {
        assert_eq!(sync_word_from_legacy(0x34), 0x3444);
        assert_eq!(sync_word_from_legacy(0x12), 0x1424);
        assert_eq!(sync_word_to_legacy(0x3444), Ok(0x34));
        assert_eq!(sync_word_to_legacy(0x1424), Ok(0x12));
        // values outside the 0xY4Z4 shape have no single-byte equivalent
        assert_eq!(sync_word_to_legacy(0x3445), Err(RadioError::InvalidSyncWord));
        assert_eq!(sync_word_to_legacy(0x0012), Err(RadioError::InvalidSyncWord));
    }
}
