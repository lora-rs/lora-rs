use crate::mod_params::*;

pub trait InterfaceVariant {
    async fn set_nss_low(&mut self) -> Result<(), RadioError>;
    async fn set_nss_high(&mut self) -> Result<(), RadioError>;
    async fn reset(&mut self) -> Result<(), RadioError>;
    async fn wait_on_busy(&mut self) -> Result<(), RadioError>;
    async fn await_irq(&mut self) -> Result<(), RadioError>;
    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError>;
    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError>;
    async fn disable_rf_switch(&mut self) -> Result<(), RadioError>;
}

pub trait RadioKind {
    fn get_radio_type(&mut self) -> RadioType;
    async fn reset(&mut self) -> Result<(), RadioError>;
    async fn ensure_ready(&mut self, mode: RadioMode) -> Result<(), RadioError>;
    async fn init_rf_switch(&mut self) -> Result<(), RadioError>;
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError>;
    async fn set_standby(&mut self) -> Result<(), RadioError>;
    async fn set_lora_modem(&mut self, enable_public_network: bool) -> Result<(), RadioError>;
    async fn set_oscillator(&mut self) -> Result<(), RadioError>;
    async fn set_regulator_mode(&mut self) -> Result<(), RadioError>;
    async fn set_tx_rx_buffer_base_address(&mut self, tx_base_addr: usize, rx_base_addr: usize) -> Result<(), RadioError>;
    async fn set_tx_power_and_ramp_time(&mut self, power: i8, is_tx_prep: bool) -> Result<(), RadioError>;
    async fn set_pa_config(&mut self, pa_duty_cycle: u8, hp_max: u8, device_sel: u8, pa_lut: u8) -> Result<(), RadioError>;
    async fn update_retention_list(&mut self) -> Result<(), RadioError>;
    async fn set_modulation_params(&mut self, mod_params: ModulationParams) -> Result<(), RadioError>;
    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError>;
    async fn calibrate_image(&mut self, frequency_in_hz: u32) -> Result<(), RadioError>;
    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError>;
    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError>;
    async fn do_tx(&mut self, timeout_in_ms: u32) -> Result<(), RadioError>;
    async fn process_irq(&mut self, radio_mode: RadioMode, receiving_buffer: Option<&mut [u8]>, received_len: Option<&mut u8>, cad_activity_detected: Option<&mut bool>) -> Result<(), RadioError>;
}
