mod radio_kind_params;

use crate::{
    mock::radio_kind_params::IrqMask,
    mod_params::{ModulationParams, PacketParams, PacketStatus, RadioError, RadioMode},
    mod_traits::{IrqState, RadioKind},
};

/// Use to check whether the radio has been set in correct state
pub enum MockState {
    /// No Op
    Standby,
    /// When set to sleep
    Sleeping,
    /// When transmitting
    Tx,
    /// When receiving
    Rx,
    /// When warm transmit
    WarmTx,
    /// When warm receiving
    WarmRx,
}

/// Struct to be used for unit testing
pub struct MockRadio {
    /// The internal state of what state the radio should be in
    pub state: MockState,
    warm_start: bool,
    irq_flags: u16,
}

impl MockRadio {
    /// Sets internal flag, to be used for checking different failure states
    pub fn set_irq_flags(&mut self, flag: u16) {
        self.irq_flags = flag;
    }
}

impl RadioKind for MockRadio {
    async fn init_lora(&mut self, _sync_word: u8) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    fn create_modulation_params(
        &self,
        spreading_factor: lora_modulation::SpreadingFactor,
        bandwidth: lora_modulation::Bandwidth,
        coding_rate: lora_modulation::CodingRate,
        frequency_in_hz: u32,
    ) -> Result<crate::mod_params::ModulationParams, crate::mod_params::RadioError> {
        Ok(ModulationParams {
            spreading_factor,
            bandwidth,
            coding_rate,
            low_data_rate_optimize: 0_u8,
            frequency_in_hz,
        })
    }

    fn create_packet_params(
        &self,
        preamble_length: u16,
        implicit_header: bool,
        payload_length: u8,
        crc_on: bool,
        iq_inverted: bool,
        _modulation_params: &crate::mod_params::ModulationParams,
    ) -> Result<crate::mod_params::PacketParams, crate::mod_params::RadioError> {
        Ok(PacketParams {
            preamble_length,
            implicit_header,
            payload_length,
            crc_on,
            iq_inverted,
        })
    }

    async fn reset(
        &mut self,
        delay: &mut impl embedded_hal_async::delay::DelayNs,
    ) -> Result<(), crate::mod_params::RadioError> {
        delay.delay_ns(10).await;
        self.irq_flags = IrqMask::None.value();
        Ok(())
    }

    async fn ensure_ready(&mut self, _mode: crate::mod_params::RadioMode) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn set_standby(&mut self) -> Result<(), crate::mod_params::RadioError> {
        self.state = MockState::Standby;
        Ok(())
    }

    async fn set_sleep(
        &mut self,
        warm_start_if_possible: bool,
        delay: &mut impl embedded_hal_async::delay::DelayNs,
    ) -> Result<(), crate::mod_params::RadioError> {
        delay.delay_ns(10).await;
        self.state = MockState::Sleeping;
        self.warm_start = warm_start_if_possible;

        Ok(())
    }

    async fn set_tx_rx_buffer_base_address(
        &mut self,
        _tx_base_addr: usize,
        _rx_base_addr: usize,
    ) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn set_tx_power_and_ramp_time(
        &mut self,
        _output_power: i32,
        _mdltn_params: Option<&crate::mod_params::ModulationParams>,
        _is_tx_prep: bool,
    ) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn set_modulation_params(
        &mut self,
        _mdltn_params: &ModulationParams,
    ) -> Result<(), crate::mod_params::RadioError> {
        // let spreading_factor_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        // let bandwidth_val = bandwidth_value(mdltn_params.bandwidth)?;
        // let coding_rate_val = coding_rate_value(mdltn_params.coding_rate?;
        // self.mod_params = mdltn_params;
        Ok(())
    }

    async fn set_packet_params(&mut self, _pkt_params: &PacketParams) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn calibrate_image(&mut self, _frequency_in_hz: u32) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn set_channel(&mut self, _frequency_in_hz: u32) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn set_payload(&mut self, _payload: &[u8]) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn do_tx(&mut self) -> Result<(), crate::mod_params::RadioError> {
        self.state = if self.warm_start {
            MockState::WarmTx
        } else {
            MockState::Tx
        };
        Ok(())
    }

    async fn do_rx(&mut self, _rx_mode: crate::RxMode) -> Result<(), crate::mod_params::RadioError> {
        self.state = if self.warm_start {
            MockState::WarmRx
        } else {
            MockState::Rx
        };
        Ok(())
    }

    async fn get_rx_payload(
        &mut self,
        rx_pkt_params: &PacketParams,
        _receiving_buffer: &mut [u8],
    ) -> Result<u8, crate::mod_params::RadioError> {
        Ok(rx_pkt_params.payload_length)
    }

    async fn get_rx_packet_status(&mut self) -> Result<crate::mod_params::PacketStatus, crate::mod_params::RadioError> {
        Ok(PacketStatus { rssi: 0, snr: 0 })
    }

    async fn get_rssi(&mut self) -> Result<i16, crate::mod_params::RadioError> {
        Ok(0)
    }

    async fn do_cad(&mut self, _mdltn_params: &ModulationParams) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn set_irq_params(
        &mut self,
        radio_mode: Option<crate::mod_params::RadioMode>,
    ) -> Result<(), crate::mod_params::RadioError> {
        let irq_mask = match radio_mode {
            Some(RadioMode::Standby) => IrqMask::All.value(),
            Some(RadioMode::Transmit) => IrqMask::TxDone.value() | IrqMask::RxTxTimeout.value(),
            Some(RadioMode::Receive(_)) => IrqMask::All.value(),
            Some(RadioMode::ChannelActivityDetection) => {
                IrqMask::CADDone.value() | IrqMask::CADActivityDetected.value()
            }
            _ => self.irq_flags,
        };
        self.irq_flags = irq_mask;

        Ok(())
    }

    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn await_irq(&mut self) -> Result<(), crate::mod_params::RadioError> {
        Ok(())
    }

    async fn process_irq_event(
        &mut self,
        radio_mode: crate::mod_params::RadioMode,
        cad_activity_detected: Option<&mut bool>,
        _clear_interrupts: bool,
    ) -> Result<Option<crate::mod_traits::IrqState>, crate::mod_params::RadioError> {
        self.get_irq_state(radio_mode, cad_activity_detected).await
    }

    async fn get_irq_state(
        &mut self,
        radio_mode: crate::mod_params::RadioMode,
        cad_activity_detected: Option<&mut bool>,
    ) -> Result<Option<crate::mod_traits::IrqState>, crate::mod_params::RadioError> {
        let irq_flags = self.irq_flags;
        match radio_mode {
            RadioMode::Transmit => {
                if IrqMask::TxDone.is_set(irq_flags) {
                    return Ok(Some(IrqState::Done));
                }
                if IrqMask::RxTxTimeout.is_set(irq_flags) {
                    return Err(RadioError::TransmitTimeout);
                }
                if irq_flags == 0 {
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Receive(_) => {
                if IrqMask::CRCError.is_set(irq_flags) || IrqMask::HeaderError.is_set(irq_flags) {
                    debug!("CRC or Header error");
                }
                if IrqMask::RxDone.is_set(irq_flags) {
                    return Ok(Some(IrqState::Done));
                }
                if IrqMask::RxTxTimeout.is_set(irq_flags) {
                    return Err(RadioError::ReceiveTimeout);
                }
                if IrqMask::PreambleDetected.is_set(irq_flags) || IrqMask::SyncwordValid.is_set(irq_flags) {
                    return Ok(Some(IrqState::PreambleReceived));
                }
            }
            RadioMode::ChannelActivityDetection => {
                if IrqMask::CADDone.is_set(irq_flags) {
                    if let Some(detected) = cad_activity_detected {
                        *detected = IrqMask::CADActivityDetected.is_set(irq_flags);
                    }
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Sleep | RadioMode::Standby | RadioMode::Listen => {
                warn!("IRQ during sleep/standby/listen?");
            }
            RadioMode::FrequencySynthesis => {}
        }

        Ok(None)
    }

    async fn clear_irq_status(&mut self) -> Result<(), crate::mod_params::RadioError> {
        self.irq_flags = IrqMask::None.value();
        Ok(())
    }
}
