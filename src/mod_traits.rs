use embedded_hal_async::delay::DelayUs;

use crate::mod_params::*;

pub trait InterfaceVariant {
    async fn set_nss_low(&mut self) -> Result<(), RadioError>;
    async fn set_nss_high(&mut self) -> Result<(), RadioError>;
    async fn reset(&mut self, delay: &mut impl DelayUs) -> Result<(), RadioError>;
    async fn wait_on_busy(&mut self) -> Result<(), RadioError>;
    async fn await_irq(&mut self) -> Result<(), RadioError>;
    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError>;
    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError>;
    async fn disable_rf_switch(&mut self) -> Result<(), RadioError>;
}

pub trait RadioKind {
    fn get_radio_type(&mut self) -> RadioType;
    async fn reset(&mut self, delay: &mut impl DelayUs) -> Result<(), RadioError>;
    async fn ensure_ready(&mut self, mode: RadioMode) -> Result<(), RadioError>;
    async fn init_rf_switch(&mut self) -> Result<(), RadioError>;
    async fn set_standby(&mut self) -> Result<(), RadioError>;
    async fn set_sleep(&mut self, delay: &mut impl DelayUs) -> Result<bool, RadioError>;
    async fn set_lora_modem(&mut self, enable_public_network: bool) -> Result<(), RadioError>;
    async fn set_oscillator(&mut self) -> Result<(), RadioError>;
    async fn set_regulator_mode(&mut self) -> Result<(), RadioError>;
    async fn set_tx_rx_buffer_base_address(
        &mut self,
        tx_base_addr: usize,
        rx_base_addr: usize,
    ) -> Result<(), RadioError>;
    async fn set_tx_power_and_ramp_time(
        &mut self,
        power: i8,
        tx_boosted_if_possible: bool,
        is_tx_prep: bool,
    ) -> Result<(), RadioError>;
    async fn update_retention_list(&mut self) -> Result<(), RadioError>;
    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError>;
    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError>;
    async fn calibrate_image(&mut self, frequency_in_hz: u32) -> Result<(), RadioError>;
    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError>;
    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError>;
    async fn do_tx(&mut self, timeout_in_ms: u32) -> Result<(), RadioError>;
    async fn do_rx(
        &mut self,
        rx_pkt_params: &PacketParams,
        duty_cycle_params: Option<&DutyCycleParams>,
        rx_continuous: bool,
        rx_boosted_if_supported: bool,
        symbol_timeout: u16,
        rx_timeout_in_ms: u32,
    ) -> Result<(), RadioError>;
    async fn get_rx_payload(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<u8, RadioError>;
    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError>;
    async fn do_cad(
        &mut self,
        mdltn_params: &ModulationParams,
        rx_boosted_if_supported: bool,
    ) -> Result<(), RadioError>;
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError>;
    async fn process_irq(
        &mut self,
        radio_mode: RadioMode,
        rx_continuous: bool,
        cad_activity_detected: Option<&mut bool>,
    ) -> Result<(), RadioError>;
}
