#![no_std]
#![allow(dead_code)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

//! lora provides a configurable LoRa physical layer for various MCU/Semtech chip combinations.

pub(crate) mod interface;
pub mod mod_params;
pub mod mod_traits;
pub mod sx1261_2;
pub mod sx1276_7_8_9;

use embedded_hal_async::delay::DelayUs;
use interface::*;
use mod_params::*;
use mod_traits::*;

/// Provides high-level access to Semtech SX126x-based boards
pub struct LoRa<RK> {
    radio_kind: RK,
    radio_mode: RadioMode,
    rx_continuous: bool,
    image_calibrated: bool,
}

impl<RK> LoRa<RK>
where
    RK: RadioKind + 'static,
{
    /// Builds and returns a new instance of the radio.
    pub async fn new(radio_kind: RK, enable_public_network: bool, delay: &mut impl DelayUs) -> Result<Self, RadioError> {
        let mut lora = Self {
            radio_kind,
            radio_mode: RadioMode::Sleep,
            rx_continuous: false,
            image_calibrated: false,
        };
        lora.init(enable_public_network, delay).await?;

        Ok(lora)
    }

    pub fn create_modulation_params(
        &mut self,
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
    ) -> Result<ModulationParams, RadioError> {
        match self.radio_kind.get_radio_type() {
            RadioType::SX1261 | RadioType::SX1262 => {
                ModulationParams::new_for_sx1261_2(spreading_factor, bandwidth, coding_rate)
            }
            RadioType::SX1276 | RadioType::SX1277 | RadioType::SX1278 | RadioType::SX1279 => {
                ModulationParams::new_for_sx1276_7_8_9(spreading_factor, bandwidth, coding_rate)
            }
        }
    }

    pub fn create_tx_packet_params(
        &mut self,
        preamble_length: u16,
        implicit_header: bool,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: ModulationParams,
    ) -> Result<PacketParams, RadioError> {
        match self.radio_kind.get_radio_type() {
            RadioType::SX1261 | RadioType::SX1262 => PacketParams::new_for_sx1261_2(
                preamble_length,
                implicit_header,
                0,
                crc_on,
                iq_inverted,
                modulation_params,
            ),
            RadioType::SX1276 | RadioType::SX1277 | RadioType::SX1278 | RadioType::SX1279 => PacketParams::new_for_sx1276_7_8_9(
                preamble_length,
                implicit_header,
                0,
                crc_on,
                iq_inverted,
                modulation_params,
            )
        }
    }

    pub fn create_rx_packet_params(
        &mut self,
        preamble_length: u16,
        implicit_header: bool,
        max_payload_length: u8,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: ModulationParams,
    ) -> Result<PacketParams, RadioError> {
        match self.radio_kind.get_radio_type() {
            RadioType::SX1261 | RadioType::SX1262 => PacketParams::new_for_sx1261_2(
                preamble_length,
                implicit_header,
                max_payload_length,
                crc_on,
                iq_inverted,
                modulation_params,
            ),
            RadioType::SX1276 | RadioType::SX1277 | RadioType::SX1278 | RadioType::SX1279 => PacketParams::new_for_sx1276_7_8_9(
                preamble_length,
                implicit_header,
                max_payload_length,
                crc_on,
                iq_inverted,
                modulation_params,
            )
        }
    }

    pub async fn init(&mut self, enable_public_network: bool, delay: &mut impl DelayUs) -> Result<(), RadioError> {
        self.image_calibrated = false;
        self.radio_kind.reset(delay).await?;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        self.radio_kind.init_rf_switch().await?;
        self.radio_kind.set_standby().await?;
        self.radio_mode = RadioMode::Standby;
        self.rx_continuous = false;
        self.radio_kind
            .set_lora_modem(enable_public_network)
            .await?;
        self.radio_kind.set_oscillator().await?;
        self.radio_kind.set_regulator_mode().await?;
        self.radio_kind.set_tx_rx_buffer_base_address(0, 0).await?;
        self.radio_kind.set_tx_power_and_ramp_time(0, false, false).await?;
        self.radio_kind
            .set_irq_params(Some(self.radio_mode))
            .await?;
        self.radio_kind.update_retention_list().await
    }

    pub async fn sleep(&mut self, delay: &mut impl DelayUs) -> Result<(), RadioError> {
        if self.radio_mode != RadioMode::Sleep {
            self.radio_kind.ensure_ready(self.radio_mode).await?;
            let warm_start_enabled = self.radio_kind.set_sleep(delay).await?;
            if !warm_start_enabled {
                self.image_calibrated = false;
            }
            self.radio_mode = RadioMode::Sleep;
        }
        Ok(())
    }

    pub async fn prepare_for_tx(
        &mut self,
        mod_params: ModulationParams,
        power: i8,
        tx_boosted_if_possible: bool
    ) -> Result<(), RadioError> {
        self.rx_continuous = false;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }
        self.radio_kind.set_modulation_params(mod_params).await?;
        self.radio_kind
            .set_tx_power_and_ramp_time(power, tx_boosted_if_possible, true)
            .await
    }

    // timeout: ms
    pub async fn tx(
        &mut self,
        tx_pkt_params: &mut PacketParams,
        frequency_in_hz: u32,
        buffer: &[u8],
        timeout_in_ms: u32,
    ) -> Result<(), RadioError> {
        self.rx_continuous = false; 
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }

        tx_pkt_params.set_payload_length(buffer.len())?;
        self.radio_kind.set_packet_params(tx_pkt_params).await?;
        if !self.image_calibrated {
            self.radio_kind.calibrate_image(frequency_in_hz).await?;
            self.image_calibrated = true;
        }
        self.radio_kind.set_channel(frequency_in_hz).await?;
        self.radio_kind.set_payload(buffer).await?;
        self.radio_mode = RadioMode::Transmit;
        self.radio_kind
            .set_irq_params(Some(self.radio_mode))
            .await?;
        self.radio_kind.do_tx(timeout_in_ms).await?;
        match self
            .radio_kind
            .process_irq(self.radio_mode, self.rx_continuous, None)
            .await
        {
            Ok(()) => {
                return Ok(());
            }
            Err(err) => {
                self.radio_kind.ensure_ready(self.radio_mode).await?;
                self.radio_kind.set_standby().await?;
                self.radio_mode = RadioMode::Standby;
                return Err(err);
            }
        }
    }

    pub async fn prepare_for_rx(
        &mut self,
        mod_params: ModulationParams,
        rx_pkt_params: &PacketParams,
        duty_cycle_params: Option<&DutyCycleParams>,
        rx_continuous: bool,
        rx_boosted_if_supported: bool,
        frequency_in_hz: u32,
        symbol_timeout: u16,
        rx_timeout_in_ms: u32
    ) -> Result<(), RadioError> {
        self.rx_continuous = rx_continuous;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }

        self.radio_kind.set_modulation_params(mod_params).await?;
        self.radio_kind.set_packet_params(rx_pkt_params).await?;
        if !self.image_calibrated {
            self.radio_kind.calibrate_image(frequency_in_hz).await?;
            self.image_calibrated = true;
        }
        self.radio_kind.set_channel(frequency_in_hz).await?;
        self.radio_mode = match duty_cycle_params {
            Some(&_duty_cycle) => RadioMode::ReceiveDutyCycle,
            None => RadioMode::Receive
        };
        self.radio_kind
            .set_irq_params(Some(self.radio_mode))
            .await?;
        self.radio_kind.do_rx(rx_pkt_params, duty_cycle_params, self.rx_continuous, rx_boosted_if_supported, symbol_timeout, rx_timeout_in_ms).await
    }
    
    pub async fn rx(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8]
    ) -> Result<(u8, PacketStatus), RadioError> {
        match self
            .radio_kind
            .process_irq(self.radio_mode, self.rx_continuous, None)
            .await
        {
            Ok(()) => {
                let received_len = self.radio_kind.get_rx_payload(rx_pkt_params, receiving_buffer).await?;
                let rx_pkt_status = self.radio_kind.get_rx_packet_status().await?;
                Ok((received_len, rx_pkt_status))
            }
            Err(err) => {
                // if in rx continuous mode, allow the caller to determine whether to keep receiving
                if !self.rx_continuous {
                    self.radio_kind.ensure_ready(self.radio_mode).await?;
                    self.radio_kind.set_standby().await?;
                    self.radio_mode = RadioMode::Standby;
                }
                Err(err)
            }
        }
    }

    pub async fn prepare_for_cad(
        &mut self,
        mod_params: ModulationParams,
        rx_boosted_if_supported: bool,
        frequency_in_hz: u32,
    ) -> Result<(), RadioError> {
        self.rx_continuous = false;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }

        self.radio_kind.set_modulation_params(mod_params).await?;
        if !self.image_calibrated {
            self.radio_kind.calibrate_image(frequency_in_hz).await?;
            self.image_calibrated = true;
        }
        self.radio_kind.set_channel(frequency_in_hz).await?;
        self.radio_mode = RadioMode::ChannelActivityDetection;
        self.radio_kind
            .set_irq_params(Some(self.radio_mode))
            .await?;
        self.radio_kind.do_cad(mod_params, rx_boosted_if_supported).await
    }
    
    pub async fn cad(
        &mut self,
    ) -> Result<bool, RadioError> {
        let mut cad_activity_detected = false;
        match self
            .radio_kind
            .process_irq(self.radio_mode, self.rx_continuous, Some(&mut cad_activity_detected))
            .await
        {
            Ok(()) => {
                Ok(cad_activity_detected)
            }
            Err(err) => {
                self.radio_kind.ensure_ready(self.radio_mode).await?;
                self.radio_kind.set_standby().await?;
                self.radio_mode = RadioMode::Standby;
                Err(err)
            }
        }
    }
}
