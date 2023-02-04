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
    pub async fn new(radio_kind: RK, enable_public_network: bool) -> Result<Self, RadioError> {
        let mut lora = Self {
            radio_kind,
            radio_mode: RadioMode::Sleep,
            rx_continuous: false,
            image_calibrated: false,
        };
        lora.init(enable_public_network).await?;

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
        }
    }

    pub async fn init(&mut self, enable_public_network: bool) -> Result<(), RadioError> {
        self.image_calibrated = false;
        self.radio_kind.reset().await?;
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
        self.radio_kind.set_tx_power_and_ramp_time(0, false).await?;
        self.radio_kind
            .set_irq_params(Some(self.radio_mode))
            .await?;
        self.radio_kind.update_retention_list().await?;

        Ok(())
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
    ) -> Result<(), RadioError> {
        self.rx_continuous = false;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }
        self.radio_kind.set_modulation_params(mod_params).await?;
        self.radio_kind
            .set_tx_power_and_ramp_time(power, true)
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
            .process_irq(self.radio_mode, self.rx_continuous)
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
        self.radio_mode = RadioMode::Receive;
        self.radio_kind
            .set_irq_params(Some(self.radio_mode))
            .await?;
        self.radio_kind.do_rx(rx_pkt_params, self.rx_continuous, rx_boosted_if_supported, symbol_timeout, rx_timeout_in_ms).await
    }
    
    pub async fn rx(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8]
    ) -> Result<(u8, PacketStatus), RadioError> {
        match self
            .radio_kind
            .process_irq(self.radio_mode, self.rx_continuous)
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

    /*

    /// Return current radio state
    pub fn get_status(&mut self) -> RadioState {
        match self.brd_get_operating_mode() {
            RadioMode::Transmit => RadioState::TxRunning,
            RadioMode::Receive => RadioState::RxRunning,
            RadioMode::ChannelActivityDetection => RadioState::ChannelActivityDetecting,
            _ => RadioState::Idle,
        }
    }

    /* Checks if the channel is free for the given time.  This is currently not implemented until a substitute
        for switching to the FSK modem is found.

    pub async fn is_channel_free(&mut self, frequency: u32, rxBandwidth: u32, rssiThresh: i16, maxCarrierSenseTime: u32) -> bool;
    */

    /// Generate a 32 bit random value based on the RSSI readings, after disabling all interrupts.   Ensure set_lora_modem() is called befrorehand.
    /// After calling this function either set_rx_config() or set_tx_config() must be called.
    pub async fn get_random_value(&mut self) -> Result<u32, RadioError> {
        self.sub_set_dio_irq_params(
            IrqMask::None.value(),
            IrqMask::None.value(),
            IrqMask::None.value(),
            IrqMask::None.value(),
        )
        .await?;

        let result = self.sub_get_random().await?;
        Ok(result)
    }

    /// Check if the given RF frequency is supported by the hardware [true: supported, false: unsupported]
    pub async fn check_rf_frequency(&mut self, frequency: u32) -> Result<bool, RadioError> {
        Ok(self.brd_check_rf_frequency(frequency).await?)
    }

    /// Computes the packet time on air in ms for the given payload for a LoRa modem (can only be called once set_rx_config or set_tx_config have been called)
    ///   spreading_factor     [6: 64, 7: 128, 8: 256, 9: 512, 10: 1024, 11: 2048, 12: 4096 chips/symbol]
    ///   bandwidth            [0: 125 kHz, 1: 250 kHz, 2: 500 kHz, 3: Reserved]
    ///   coding_rate          [1: 4/5, 2: 4/6, 3: 4/7, 4: 4/8]
    ///   preamble_length      length in symbols (the hardware adds 4 more symbols)
    ///   fixed_len            fixed length packets [0: variable, 1: fixed]
    ///   payload_len          sets payload length when fixed length is used
    ///   crc_on               [0: OFF, 1: ON]
    pub fn get_time_on_air(
        &mut self,
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        preamble_length: u16,
        fixed_len: bool,
        payload_len: u8,
        crc_on: bool,
    ) -> Result<u32, RadioError> {
        let numerator = 1000
            * Self::get_lora_time_on_air_numerator(
                spreading_factor,
                bandwidth,
                coding_rate,
                preamble_length,
                fixed_len,
                payload_len,
                crc_on,
            );
        let denominator = bandwidth.value_in_hz();
        if denominator == 0 {
            Err(RadioError::InvalidBandwidth)
        } else {
            Ok((numerator + denominator - 1) / denominator)
        }
    }

    /// Start a Channel Activity Detection
    pub async fn start_cad(&mut self) -> Result<(), RadioError> {
        self.sub_set_dio_irq_params(
            IrqMask::CADDone.value() | IrqMask::CADActivityDetected.value(),
            IrqMask::CADDone.value() | IrqMask::CADActivityDetected.value(),
            IrqMask::None.value(),
            IrqMask::None.value(),
        )
        .await?;
        self.sub_set_cad().await?;
        Ok(())
    }

    /// Sets the radio in continuous wave transmission mode
    ///   frequency    channel RF frequency
    ///   power        output power [dBm]
    ///   timeout      transmission mode timeout [s]
    pub async fn set_tx_continuous_wave(
        &mut self,
        frequency: u32,
        power: i8,
        _timeout: u16,
    ) -> Result<(), RadioError> {
        self.sub_set_rf_frequency(frequency).await?;
        self.brd_set_rf_tx_power(power).await?;
        self.sub_set_tx_continuous_wave().await?;

        Ok(())
    }

    /// Read the current RSSI value for the LoRa modem (only) [dBm]
    pub async fn get_rssi(&mut self) -> Result<i16, RadioError> {
        let value = self.sub_get_rssi_inst().await?;
        Ok(value as i16)
    }

    /// Write one or more radio registers with a buffer of a given size, starting at the first register address
    pub async fn write_registers_from_buffer(
        &mut self,
        start_register: Register,
        buffer: &[u8],
    ) -> Result<(), RadioError> {
        self.brd_write_registers(start_register, buffer).await?;
        Ok(())
    }

    /// Read one or more radio registers into a buffer of a given size, starting at the first register address
    pub async fn read_registers_into_buffer(
        &mut self,
        start_register: Register,
        buffer: &mut [u8],
    ) -> Result<(), RadioError> {
        self.brd_read_registers(start_register, buffer).await?;
        Ok(())
    }

    /// Set the maximum payload length (in bytes) for a LoRa modem (only).
    pub async fn set_max_payload_length(&mut self, max: u8) -> Result<(), RadioError> {
        if self.packet_params.is_some() {
            let packet_params = self.packet_params.as_mut().unwrap();
            // self.max_payload_length = max;
            packet_params.payload_length = max;
            self.sub_set_packet_params().await?;
            Ok(())
        } else {
            Err(RadioError::PacketParamsMissing)
        }
    }

    // SX126x-specific functions

    /// Set the Rx duty cycle management parameters (SX126x radios only)
    ///   rx_time       structure describing reception timeout value
    ///   sleep_time    structure describing sleep timeout value
    pub async fn set_rx_duty_cycle(
        &mut self,
        rx_time: u32,
        sleep_time: u32,
    ) -> Result<(), RadioError> {
        self.sub_set_rx_duty_cycle(rx_time, sleep_time).await?;
        Ok(())
    }

    // Utilities

    fn get_lora_time_on_air_numerator(
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        preamble_length: u16,
        fixed_len: bool,
        payload_len: u8,
        crc_on: bool,
    ) -> u32 {
        let cell_denominator;
        let cr_denominator = (coding_rate.value() as i32) + 4;

        // Ensure that the preamble length is at least 12 symbols when using SF5 or SF6
        let mut preamble_length_final = preamble_length;
        if ((spreading_factor == SpreadingFactor::_5) || (spreading_factor == SpreadingFactor::_6))
            && (preamble_length < 12)
        {
            preamble_length_final = 12;
        }

        let mut low_data_rate_optimize = false;
        if (((spreading_factor == SpreadingFactor::_11)
            || (spreading_factor == SpreadingFactor::_12))
            && (bandwidth == Bandwidth::_125KHz))
            || ((spreading_factor == SpreadingFactor::_12) && (bandwidth == Bandwidth::_250KHz))
        {
            low_data_rate_optimize = true;
        }

        let mut cell_numerator = ((payload_len as i32) << 3) + (if crc_on { 16 } else { 0 })
            - (4 * spreading_factor.value() as i32)
            + (if fixed_len { 0 } else { 20 });

        if spreading_factor.value() <= 6 {
            cell_denominator = 4 * (spreading_factor.value() as i32);
        } else {
            cell_numerator += 8;
            if low_data_rate_optimize {
                cell_denominator = 4 * ((spreading_factor.value() as i32) - 2);
            } else {
                cell_denominator = 4 * (spreading_factor.value() as i32);
            }
        }

        if cell_numerator < 0 {
            cell_numerator = 0;
        }

        let mut intermediate: i32 = (((cell_numerator + cell_denominator - 1) / cell_denominator)
            * cr_denominator)
            + (preamble_length_final as i32)
            + 12;

        if spreading_factor.value() <= 6 {
            intermediate = intermediate + 2;
        }

        (((4 * intermediate) + 1) * (1 << (spreading_factor.value() - 2))) as u32
    }
    */
}
