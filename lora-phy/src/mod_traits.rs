use embedded_hal_async::delay::DelayNs;

use crate::mod_params::*;

/// Functions implemented for an embedded framework for an MCU/LoRa chip combination
/// to allow this crate to control the LoRa chip.
pub trait InterfaceVariant {
    /// Reset the LoRa chip
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError>;
    /// Wait for the LoRa chip to become available for an operation
    async fn wait_on_busy(&mut self) -> Result<(), RadioError>;
    /// Wait for the LoRa chip to indicate an event has occurred
    async fn await_irq(&mut self) -> Result<(), RadioError>;
    /// Enable an antenna used for receive operations, disabling other antennas
    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError>;
    /// Enable an antenna used for send operations, disabling other antennas
    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError>;
    /// Disable all antennas
    async fn disable_rf_switch(&mut self) -> Result<(), RadioError>;
}

/// Specifies an IRQ processing state to run the loop to
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetIrqState {
    /// Runs the loop until after the preamble has been received
    PreambleReceived,
    /// Runs the loop until the operation is fully complete
    Done,
}

/// An actual operation state, including some details where necessary
#[derive(Clone, Copy)]
pub enum IrqState {
    /// Preamble has been received
    PreambleReceived,
    /// The RX operation is complete
    RxDone(u8, PacketStatus),
}

/// Functions implemented for a specific kind of LoRa chip, called internally by the outward facing
/// LoRa physical layer API
pub trait RadioKind {
    /// Create modulation parameters specific to the LoRa chip kind and type
    fn create_modulation_params(
        &self,
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        frequency_in_hz: u32,
    ) -> Result<ModulationParams, RadioError>;
    /// Create packet parameters specific to the LoRa chip kind and type
    fn create_packet_params(
        &self,
        preamble_length: u16,
        implicit_header: bool,
        payload_length: u8,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: &ModulationParams,
    ) -> Result<PacketParams, RadioError>;
    /// Reset the loRa chip
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError>;
    /// Ensure the LoRa chip is in the appropriate state to allow operation requests
    async fn ensure_ready(&mut self, mode: RadioMode) -> Result<(), RadioError>;
    /// Perform any necessary antenna initialization
    async fn init_rf_switch(&mut self) -> Result<(), RadioError>;
    /// Place the LoRa chip in standby mode
    async fn set_standby(&mut self) -> Result<(), RadioError>;
    /// Place the LoRa chip in power-saving mode
    async fn set_sleep(&mut self, warm_start_if_possible: bool, delay: &mut impl DelayNs) -> Result<(), RadioError>;
    /// Perform operations to set a multi-protocol chip as a LoRa chip
    async fn set_lora_modem(&mut self, enable_public_network: bool) -> Result<(), RadioError>;
    /// Perform operations to set the LoRa chip oscillator
    async fn set_oscillator(&mut self) -> Result<(), RadioError>;
    /// Set the LoRa chip voltage regulator mode
    async fn set_regulator_mode(&mut self) -> Result<(), RadioError>;
    /// Set the LoRa chip send and receive buffer base addresses
    async fn set_tx_rx_buffer_base_address(
        &mut self,
        tx_base_addr: usize,
        rx_base_addr: usize,
    ) -> Result<(), RadioError>;
    /// Perform any necessary LoRa chip power setup prior to a send operation
    async fn set_tx_power_and_ramp_time(
        &mut self,
        output_power: i32,
        mdltn_params: Option<&ModulationParams>,
        tx_boosted_if_possible: bool,
        is_tx_prep: bool,
    ) -> Result<(), RadioError>;
    /// Update the LoRa chip retention list to support warm starts from sleep
    async fn update_retention_list(&mut self) -> Result<(), RadioError>;
    /// Set the LoRa chip modulation parameters prior to using a communication channel
    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError>;
    /// Set the LoRa chip packet parameters prior to sending or receiving packets
    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError>;
    /// Set the LoRa chip to support a given communication channel frequency
    async fn calibrate_image(&mut self, frequency_in_hz: u32) -> Result<(), RadioError>;
    /// Set the frequency for a communication channel
    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError>;
    /// Set a payload for a subsequent send operation
    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError>;
    /// Perform a send operation
    async fn do_tx(&mut self, timeout_in_ms: u32) -> Result<(), RadioError>;
    /// Set up to perform a receive operation (single-shot, continuous, or duty cycle)
    async fn do_rx(
        &mut self,
        rx_pkt_params: &PacketParams,
        duty_cycle_params: Option<&DutyCycleParams>,
        rx_continuous: bool,
        rx_boosted_if_supported: bool,
        symbol_timeout: u16,
    ) -> Result<(), RadioError>;
    /// Get an available packet made available as the result of a receive operation
    async fn get_rx_payload(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<u8, RadioError>;
    /// Get the RSSI and SNR for the packet made available as the result of a receive operation
    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError>;
    /// Perform a channel activity detection operation
    async fn do_cad(
        &mut self,
        mdltn_params: &ModulationParams,
        rx_boosted_if_supported: bool,
    ) -> Result<(), RadioError>;
    /// Set the LoRa chip to provide notification of specific events based on radio state
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError>;
    /// Process LoRa chip notifications of events
    async fn process_irq(
        &mut self,
        radio_mode: RadioMode,
        rx_continuous: bool,
        target_rx_state: TargetIrqState,
        delay: &mut impl DelayNs,
        polling_timeout_in_ms: Option<u32>,
        cad_activity_detected: Option<&mut bool>,
    ) -> Result<TargetIrqState, RadioError>;
    /// Set the LoRa chip into the TxContinuousWave mode
    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), RadioError>;
}

/// Internal trait for specifying that a [`RadioKind`] object has RNG capability.
pub(crate) trait RngRadio: RadioKind {
    async fn get_random_number(&mut self) -> Result<u32, RadioError>;
}

/// If the LoRa chip supports it, provides an async implementation of the onboard RNG. This trait makes no guarantees
/// with regards to the distribution of the generated random numbers (ie, uniform or Gaussian). If uniformity is
/// needed, apply necessary software processing.
pub trait AsyncRng {
    /// Generate a 32 bit random value.
    ///
    /// # Warning
    ///
    /// `prepare_for_xxx()` MUST be called after this operation to set modulation and packet parameters (for
    /// example: xxx = tx, rx, cad).
    /// Do not set modulation and packet parameters, do a random number generation, then initiate Tx, Rx, or CAD.
    async fn get_random_number(&mut self) -> Result<u32, RadioError>;
}
