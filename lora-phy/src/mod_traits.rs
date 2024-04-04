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
pub enum IrqState {
    /// Runs the loop until after the preamble has been received
    PreambleReceived,
    /// Runs the loop until the operation is fully complete
    Done,
}

/// Functions implemented for a specific kind of LoRa chip, called internally by the outward facing
/// LoRa physical layer API
pub trait RadioKind {
    /// Initialize lora radio
    async fn init_lora(&mut self, is_public_network: bool) -> Result<(), RadioError>;
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
    /// Place the LoRa chip in standby mode
    async fn set_standby(&mut self) -> Result<(), RadioError>;
    /// Place the LoRa chip in power-saving mode
    async fn set_sleep(&mut self, warm_start_if_possible: bool, delay: &mut impl DelayNs) -> Result<(), RadioError>;
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
        is_tx_prep: bool,
    ) -> Result<(), RadioError>;
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
    /// Perform a transmit operation
    async fn do_tx(&mut self) -> Result<(), RadioError>;
    /// Set up to perform a receive operation (single-shot, continuous, or duty cycle)
    async fn do_rx(&mut self, rx_mode: RxMode) -> Result<(), RadioError>;
    /// Get an available packet made available as the result of a receive operation
    async fn get_rx_payload(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<u8, RadioError>;
    /// Get the RSSI and SNR for the packet made available as the result of a receive operation
    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError>;
    /// Perform a channel activity detection operation
    async fn do_cad(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError>;
    /// Set the LoRa chip to provide notification of specific events based on radio state
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError>;
    /// Set the LoRa chip into the TxContinuousWave mode
    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), RadioError>;

    /// Await for an IRQ event. This is droppable and thus safe to use in a select branch.
    async fn await_irq(&mut self) -> Result<(), RadioError>;
    /// Process LoRa radio IRQs
    async fn process_irq_event(
        &mut self,
        radio_mode: RadioMode,
        cad_activity_detected: Option<&mut bool>,
        clear_interrupts: bool,
    ) -> Result<Option<IrqState>, RadioError>;
}
