#![no_std]
#![allow(dead_code)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

//! lora provides a configurable LoRa physical layer for various MCU/Semtech chip combinations.

pub mod sx1261_2;
pub mod sx1276_7_8_9;
pub(crate) mod fmt;
pub mod mod_params;
pub mod mod_traits;
pub(crate) mod interface;

use interface::*;
use mod_params::*;
use mod_traits::*;

/// Provides high-level access to Semtech SX126x-based boards
pub struct LoRa<RK> {
    radio_kind: RK,
    radio_mode: RadioMode,
    rx_continuous: bool,
    packet_status: Option<PacketStatus>,
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
            packet_status: None,
            image_calibrated: false,
        };
        lora.init(enable_public_network).await?;
        
        Ok(lora)
    }

    pub fn create_modulation_params(&mut self, spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate) -> Result<ModulationParams, RadioError> {
            match self.radio_kind.get_radio_type() {
                RadioType::SX1261 | RadioType::SX1262 => ModulationParams::new_for_sx1261_2(spreading_factor, bandwidth, coding_rate)
            }
    }

    pub fn create_rx_packet_params(&mut self, preamble_length: u16,
        implicit_header: bool,
        max_payload_length: u8,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: ModulationParams) -> Result<PacketParams, RadioError> {
            match self.radio_kind.get_radio_type() {
                RadioType::SX1261 | RadioType::SX1262 => PacketParams::new_for_sx1261_2(preamble_length, implicit_header, max_payload_length, crc_on, iq_inverted, modulation_params)
            }
    }

    pub fn create_tx_packet_params(&mut self, preamble_length: u16,
        implicit_header: bool,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: ModulationParams) -> Result<PacketParams, RadioError> {
            match self.radio_kind.get_radio_type() {
                RadioType::SX1261 | RadioType::SX1262 => PacketParams::new_for_sx1261_2(preamble_length, implicit_header, 0, crc_on, iq_inverted, modulation_params)
            }
    }

    pub async fn init(&mut self, enable_public_network: bool) -> Result<(), RadioError> {
        self.image_calibrated = false;
        self.radio_kind.reset().await?;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        self.radio_kind.init_rf_switch().await?;
        self.radio_kind.set_standby().await?;
        self.radio_mode = RadioMode::Standby;
        self.radio_kind.set_lora_modem(enable_public_network).await?;
        self.radio_kind.set_oscillator().await?;
        self.radio_kind.set_regulator_mode().await?;
        self.radio_kind.set_tx_rx_buffer_base_address(0, 0).await?;
        self.radio_kind.set_tx_power_and_ramp_time(0, false).await?;
        self.radio_kind.update_retention_list().await?;

        Ok(())
    }

    pub async fn set_tx_config(&mut self, mod_params: ModulationParams, power: i8)  -> Result<(), RadioError> {
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
        }
        self.radio_kind.set_modulation_params(mod_params).await?;
        self.radio_kind.set_tx_power_and_ramp_time(power, true).await
    }

    // timeout: ms
    pub async fn tx(&mut self, tx_pkt_params: &mut PacketParams, frequency_in_hz: u32, buffer: &[u8], timeout_in_ms: u32) -> Result<(), RadioError> {
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
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
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        self.radio_kind.do_tx(timeout_in_ms).await?;
        match self.radio_kind.process_irq(self.radio_mode, None, None, None).await {
            Ok(()) => {
                self.radio_mode = RadioMode::Standby;  // chip IRQ processing returns internal state to Standby
                return Ok(());
            },
            Err(err) => {
                self.radio_kind.ensure_ready(self.radio_mode).await?;
                self.radio_kind.set_standby().await?;
                self.radio_mode = RadioMode::Standby;
                return Err(err);
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

    /// Set the reception parameters for the LoRa modem (only).  Ensure set_lora_modem() is called befrorehand.
    ///   spreading_factor     [6: 64, 7: 128, 8: 256, 9: 512, 10: 1024, 11: 2048, 12: 4096 chips/symbol]
    ///   bandwidth            [0: 125 kHz, 1: 250 kHz, 2: 500 kHz, 3: Reserved]
    ///   coding_rate          [1: 4/5, 2: 4/6, 3: 4/7, 4: 4/8]
    ///   preamble_length      length in symbols (the hardware adds 4 more symbols)
    ///   symb_timeout         RxSingle timeout value in symbols
    ///   fixed_len            fixed length packets [0: variable, 1: fixed]
    ///   payload_len          payload length when fixed length is used
    ///   crc_on               [0: OFF, 1: ON]
    ///   freq_hop_on          intra-packet frequency hopping [0: OFF, 1: ON]
    ///   hop_period           number of symbols between each hop
    ///   iq_inverted          invert IQ signals [0: not inverted, 1: inverted]
    ///   rx_continuous        reception mode [false: single mode, true: continuous mode]
    pub async fn set_rx_config(
        &mut self,
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        preamble_length: u16,
        symb_timeout: u16,
        fixed_len: bool,
        payload_len: u8,
        crc_on: bool,
        _freq_hop_on: bool,
        _hop_period: u8,
        iq_inverted: bool,
        rx_continuous: bool,
    ) -> Result<(), RadioError> {
        let mut symb_timeout_final = symb_timeout;

        self.rx_continuous = rx_continuous;
        if self.rx_continuous {
            symb_timeout_final = 0;
        }
        /*
        if fixed_len {
            self.max_payload_length = payload_len;
        } else {
            self.max_payload_length = 0xFFu8;
        }
        */

        self.sub_set_stop_rx_timer_on_preamble_detect(false).await?;

        let mut low_data_rate_optimize = 0x00u8;
        if (((spreading_factor == SpreadingFactor::_11)
            || (spreading_factor == SpreadingFactor::_12))
            && (bandwidth == Bandwidth::_125KHz))
            || ((spreading_factor == SpreadingFactor::_12) && (bandwidth == Bandwidth::_250KHz))
        {
            low_data_rate_optimize = 0x01u8;
        }

        let modulation_params = ModulationParams {
            spreading_factor: spreading_factor,
            bandwidth: bandwidth,
            coding_rate: coding_rate,
            low_data_rate_optimize: low_data_rate_optimize,
        };

        let mut preamble_length_final = preamble_length;
        if ((spreading_factor == SpreadingFactor::_5) || (spreading_factor == SpreadingFactor::_6))
            && (preamble_length < 12)
        {
            preamble_length_final = 12;
        }

        let packet_params = PacketParams {
            preamble_length: preamble_length_final,
            implicit_header: fixed_len,
            payload_length: 0xff, // self.max_payload_length,
            max_payload_length: 0xFFu8,
            crc_on: crc_on,
            iq_inverted: iq_inverted,
        };

        self.modulation_params = Some(modulation_params);
        self.packet_params = Some(packet_params);

        self.standby().await?;
        self.sub_set_modulation_params().await?;
        self.sub_set_packet_params().await?;
        self.sub_set_lora_symb_num_timeout(symb_timeout_final)
            .await?;

        // Optimize the Inverted IQ Operation (see DS_SX1261-2_V1.2 datasheet chapter 15.4)
        let mut iq_polarity = [0x00u8];
        self.brd_read_registers(Register::IQPolarity, &mut iq_polarity)
            .await?;
        if iq_inverted {
            self.brd_write_registers(Register::IQPolarity, &[iq_polarity[0] & (!(1 << 2))])
                .await?;
        } else {
            self.brd_write_registers(Register::IQPolarity, &[iq_polarity[0] | (1 << 2)])
                .await?;
        }
        Ok(())
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

    /// Set the radio in sleep mode
    pub async fn sleep(&mut self, delay: &mut impl DelayUs) -> Result<(), RadioError> {
        self.sub_set_sleep(SleepParams {
            wakeup_rtc: false,
            reset: false,
            warm_start: true,
        })
        .await?;
        delay.delay_ms(2).await.map_err(|_| DelayError)?;
        Ok(())
    }

    /// Set the radio in reception mode for the given duration [0: continuous, others: timeout (ms)]
    pub async fn rx(&mut self, timeout: u32) -> Result<(), RadioError> {
        self.sub_set_dio_irq_params(
            IrqMask::All.value(),
            IrqMask::All.value(),
            IrqMask::None.value(),
            IrqMask::None.value(),
        )
        .await?;

        if self.rx_continuous {
            self.sub_set_rx(0xFFFFFF).await?;
        } else {
            self.sub_set_rx(timeout << 6).await?;
        }

        Ok(())
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

    /// Set the radio in reception mode with Max LNA gain for the given time (SX126x radios only) [0: continuous, others timeout in ms]
    pub async fn set_rx_boosted(&mut self, timeout: u32) -> Result<(), RadioError> {
        self.sub_set_dio_irq_params(
            IrqMask::All.value(),
            IrqMask::All.value(),
            IrqMask::None.value(),
            IrqMask::None.value(),
        )
        .await?;

        if self.rx_continuous {
            self.sub_set_rx_boosted(0xFFFFFF).await?; // Rx continuous
        } else {
            self.sub_set_rx_boosted(timeout << 6).await?;
        }

        Ok(())
    }

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

    pub fn get_latest_packet_status(&mut self) -> Option<PacketStatus> {
        self.packet_status
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
