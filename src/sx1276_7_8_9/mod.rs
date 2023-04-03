mod radio_kind_params;

use defmt::debug;
use embedded_hal_async::delay::DelayUs;
use embedded_hal_async::spi::*;
use radio_kind_params::*;

use crate::mod_params::*;
use crate::{InterfaceVariant, RadioKind, SpiInterface};

// Syncwords for public and private networks
const LORA_MAC_PUBLIC_SYNCWORD: u8 = 0x34; // corresponds to sx126x 0x3444
const LORA_MAC_PRIVATE_SYNCWORD: u8 = 0x12; // corresponds to sx126x 0x1424

// TCXO flag
const TCXO_FOR_OSCILLATOR: u8 = 0x10u8;

// Frequency synthesizer step for frequency calculation (Hz)
const FREQUENCY_SYNTHESIZER_STEP: f64 = 61.03515625; // FXOSC (32 MHz) * 1000000 (Hz/MHz) / 524288 (2^19)

impl ModulationParams {
    /// Create modulation parameters specific to the LoRa chip kind and type
    pub fn new_for_sx1276_7_8_9(
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        frequency_in_hz: u32,
    ) -> Result<Self, RadioError> {
        // Parameter validation
        spreading_factor_value(spreading_factor)?;
        bandwidth_value(bandwidth)?;
        coding_rate_value(coding_rate)?;
        if ((bandwidth == Bandwidth::_250KHz) || (bandwidth == Bandwidth::_500KHz)) && (frequency_in_hz < 400_000_000) {
            return Err(RadioError::InvalidBandwidthForFrequency);
        }

        // Section 4.1.1.5 and 4.1.1.6
        let bw_in_hz = bandwidth.value_in_hz();
        let symbol_duration = 1000 / (bw_in_hz / (0x01u32 << spreading_factor_value(spreading_factor)?));
        let mut low_data_rate_optimize = 0x00u8;
        if symbol_duration > 16 {
            low_data_rate_optimize = 0x01u8
        }

        Ok(Self {
            spreading_factor,
            bandwidth,
            coding_rate,
            low_data_rate_optimize,
            frequency_in_hz,
        })
    }
}

impl PacketParams {
    /// Create packet parameters specific to the LoRa chip kind and type
    pub fn new_for_sx1276_7_8_9(
        preamble_length: u16,
        implicit_header: bool,
        payload_length: u8,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: &ModulationParams,
    ) -> Result<Self, RadioError> {
        // Parameter validation
        if (modulation_params.spreading_factor == SpreadingFactor::_6) && !implicit_header {
            return Err(RadioError::InvalidSF6ExplicitHeaderRequest);
        }

        Ok(Self {
            preamble_length,
            implicit_header,
            payload_length,
            crc_on,
            iq_inverted,
        })
    }
}

/// Base for the RadioKind implementation for the LoRa chip kind and board type
pub struct SX1276_7_8_9<SPI, IV> {
    board_type: BoardType,
    intf: SpiInterface<SPI, IV>,
}

impl<SPI, IV> SX1276_7_8_9<SPI, IV>
where
    SPI: SpiBus<u8> + 'static,
    IV: InterfaceVariant + 'static,
{
    /// Create an instance of the RadioKind implementation for the LoRa chip kind and board type
    pub fn new(board_type: BoardType, spi: SPI, mut iv: IV) -> Self {
        iv.set_board_type(board_type);
        let intf = SpiInterface::new(spi, iv);
        Self { board_type, intf }
    }

    // Utility functions
    async fn write_register(
        &mut self,
        register: Register,
        value: u8,
        is_sleep_command: bool,
    ) -> Result<(), RadioError> {
        let write_buffer = [register.write_addr(), value];
        self.intf.write(&[&write_buffer], is_sleep_command).await
    }

    async fn read_register(&mut self, register: Register) -> Result<u8, RadioError> {
        let write_buffer = [register.read_addr()];
        let mut read_buffer = [0x00u8];
        self.intf.read(&[&write_buffer], &mut read_buffer, None).await?;
        Ok(read_buffer[0])
    }

    // Set the number of symbols the radio will wait to validate a reception
    async fn set_lora_symbol_num_timeout(&mut self, symbol_num: u8) -> Result<(), RadioError> {
        self.write_register(Register::RegSymbTimeoutLsb, symbol_num, false)
            .await
    }

    // Set the over current protection (mA) on the radio
    async fn set_ocp(&mut self, ocp_trim: OcpTrim) -> Result<(), RadioError> {
        self.write_register(Register::RegOcp, ocp_trim.value(), false).await
    }
}

impl<SPI, IV> RadioKind for SX1276_7_8_9<SPI, IV>
where
    SPI: SpiBus<u8> + 'static,
    IV: InterfaceVariant + 'static,
{
    fn get_board_type(&self) -> BoardType {
        self.board_type
    }

    async fn reset(&mut self, delay: &mut impl DelayUs) -> Result<(), RadioError> {
        self.intf.iv.reset(delay).await?;
        self.set_sleep(delay).await?; // ensure sleep mode is entered so that the LoRa mode bit is set
        Ok(())
    }

    async fn ensure_ready(&mut self, _mode: RadioMode) -> Result<(), RadioError> {
        Ok(())
    }

    // Use DIO2 to control an RF Switch
    async fn init_rf_switch(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_standby(&mut self) -> Result<(), RadioError> {
        self.write_register(Register::RegOpMode, LoRaMode::Standby.value(), false)
            .await?;
        self.intf.iv.disable_rf_switch().await
    }

    async fn set_sleep(&mut self, _delay: &mut impl DelayUs) -> Result<bool, RadioError> {
        self.intf.iv.disable_rf_switch().await?;
        self.write_register(Register::RegOpMode, LoRaMode::Sleep.value(), true)
            .await?;
        Ok(false) // warm start unavailable for sx127x
    }

    /// The sx127x LoRa mode is set when setting a mode while in sleep mode.
    async fn set_lora_modem(&mut self, enable_public_network: bool) -> Result<(), RadioError> {
        if enable_public_network {
            self.write_register(Register::RegSyncWord, LORA_MAC_PUBLIC_SYNCWORD, false)
                .await
        } else {
            self.write_register(Register::RegSyncWord, LORA_MAC_PRIVATE_SYNCWORD, false)
                .await
        }
    }

    async fn set_oscillator(&mut self) -> Result<(), RadioError> {
        self.write_register(Register::RegTcxo, TCXO_FOR_OSCILLATOR, false).await
    }

    async fn set_regulator_mode(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_tx_rx_buffer_base_address(
        &mut self,
        tx_base_addr: usize,
        rx_base_addr: usize,
    ) -> Result<(), RadioError> {
        if tx_base_addr > 255 || rx_base_addr > 255 {
            return Err(RadioError::InvalidBaseAddress(tx_base_addr, rx_base_addr));
        }
        self.write_register(Register::RegFifoTxBaseAddr, 0x00u8, false).await?;
        self.write_register(Register::RegFifoRxBaseAddr, 0x00u8, false).await
    }

    // Set parameters associated with power for a send operation.
    //   p_out                   desired RF output power (dBm)
    //   mdltn_params            needed for a power vs channel frequency validation
    //   tx_boosted_if_possible  determine if transmit boost is requested
    //   is_tx_prep              indicates which ramp up time to use
    async fn set_tx_power_and_ramp_time(
        &mut self,
        p_out: i32,
        _mdltn_params: Option<&ModulationParams>,
        tx_boosted_if_possible: bool,
        is_tx_prep: bool,
    ) -> Result<(), RadioError> {
        if tx_boosted_if_possible {
            if (p_out < 2) || (p_out > 20) {
                return Err(RadioError::InvalidOutputPower);
            }

            // Pout=17-(15-OutputPower)
            let output_power: i32 = p_out - 2;

            if p_out > 17 {
                self.write_register(Register::RegPaDac, PaDac::_20DbmOn.value(), false)
                    .await?;
                self.set_ocp(OcpTrim::_240Ma).await?;
            } else {
                self.write_register(Register::RegPaDac, PaDac::_20DbmOff.value(), false)
                    .await?;
                self.set_ocp(OcpTrim::_100Ma).await?;
            }
            self.write_register(
                Register::RegPaConfig,
                PaConfig::PaBoost.value() | (output_power as u8),
                false,
            )
            .await?;
        } else {
            if (p_out < -4) || (p_out > 14) {
                return Err(RadioError::InvalidOutputPower);
            }

            // Pmax=10.8+0.6*MaxPower, where MaxPower is set below as 7 and therefore Pmax is 15
            // Pout=Pmax-(15-OutputPower)
            let output_power: i32 = p_out;

            self.write_register(Register::RegPaDac, PaDac::_20DbmOff.value(), false)
                .await?;
            self.set_ocp(OcpTrim::_100Ma).await?;
            self.write_register(
                Register::RegPaConfig,
                PaConfig::MaxPower7NoPaBoost.value() | (output_power as u8),
                false,
            )
            .await?;
        }

        let ramp_time = match is_tx_prep {
            true => RampTime::Ramp40Us,   // for instance, prior to TX or CAD
            false => RampTime::Ramp250Us, // for instance, on initialization
        };
        self.write_register(Register::RegPaRamp, ramp_time.value(), false).await
    }

    async fn update_retention_list(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        let spreading_factor_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let bandwidth_val = bandwidth_value(mdltn_params.bandwidth)?;
        let coding_rate_denominator_val = coding_rate_denominator_value(mdltn_params.coding_rate)?;
        let mut ldro_agc_auto_flags = 0x00u8; // LDRO and AGC Auto both off
        if mdltn_params.low_data_rate_optimize != 0 {
            ldro_agc_auto_flags = 0x08u8; // LDRO on and AGC Auto off
        }

        let mut optimize = 0xc3u8;
        let mut threshold = 0x0au8;
        if mdltn_params.spreading_factor == SpreadingFactor::_6 {
            optimize = 0xc5u8;
            threshold = 0x0cu8;
        }
        self.write_register(Register::RegDetectionOptimize, optimize, false)
            .await?;
        self.write_register(Register::RegDetectionThreshold, threshold, false)
            .await?;

        let mut config_2 = self.read_register(Register::RegModemConfig2).await?;
        config_2 = (config_2 & 0x0fu8) | ((spreading_factor_val << 4) & 0xf0u8);
        self.write_register(Register::RegModemConfig2, config_2, false).await?;

        let mut config_1 = self.read_register(Register::RegModemConfig1).await?;
        config_1 = (config_1 & 0x0fu8) | ((bandwidth_val << 4) as u8);
        self.write_register(Register::RegModemConfig1, config_1, false).await?;

        let cr = coding_rate_denominator_val - 4;
        config_1 = self.read_register(Register::RegModemConfig1).await?;
        config_1 = (config_1 & 0xf1u8) | ((cr << 1) as u8);
        self.write_register(Register::RegModemConfig1, config_1, false).await?;

        let mut config_3 = self.read_register(Register::RegModemConfig3).await?;
        config_3 = (config_3 & 0xf3u8) | ldro_agc_auto_flags;
        self.write_register(Register::RegModemConfig3, config_3, false).await
    }

    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError> {
        // handle payload_length ???
        self.write_register(
            Register::RegPreambleMsb,
            ((pkt_params.preamble_length >> 8) & 0x00ff) as u8,
            false,
        )
        .await?;
        self.write_register(
            Register::RegPreambleLsb,
            (pkt_params.preamble_length & 0x00ff) as u8,
            false,
        )
        .await?;

        let mut config_1 = self.read_register(Register::RegModemConfig1).await?;
        if pkt_params.implicit_header {
            config_1 = config_1 | 0x01u8;
        } else {
            config_1 = config_1 & 0xfeu8;
        }
        self.write_register(Register::RegModemConfig1, config_1, false).await?;

        let mut config_2 = self.read_register(Register::RegModemConfig2).await?;
        if pkt_params.crc_on {
            config_2 = config_2 | 0x04u8;
        } else {
            config_2 = config_2 & 0xfbu8;
        }
        self.write_register(Register::RegModemConfig2, config_2, false).await?;

        let mut invert_iq = 0x27u8;
        let mut invert_iq2 = 0x1du8;
        if pkt_params.iq_inverted {
            invert_iq = 0x66u8;
            invert_iq2 = 0x19u8;
        }
        self.write_register(Register::RegInvertiq, invert_iq, false).await?;
        self.write_register(Register::RegInvertiq2, invert_iq2, false).await
    }

    // Calibrate the image rejection based on the given frequency
    async fn calibrate_image(&mut self, _frequency_in_hz: u32) -> Result<(), RadioError> {
        // An automatic process, but can set bit ImageCalStart in RegImageCal, when the device is in Standby mode.
        Ok(())
    }

    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        let frf = (frequency_in_hz as f64 / FREQUENCY_SYNTHESIZER_STEP) as u32;
        self.write_register(Register::RegFrfMsb, ((frf & 0x00FF0000) >> 16) as u8, false)
            .await?;
        self.write_register(Register::RegFrfMid, ((frf & 0x0000FF00) >> 8) as u8, false)
            .await?;
        self.write_register(Register::RegFrfLsb, (frf & 0x000000FF) as u8, false)
            .await
    }

    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        self.write_register(Register::RegFifoAddrPtr, 0x00u8, false).await?;
        self.write_register(Register::RegPayloadLength, 0x00u8, false).await?;
        for byte in payload {
            self.write_register(Register::RegFifo, *byte, false).await?;
        }
        self.write_register(Register::RegPayloadLength, payload.len() as u8, false)
            .await
    }

    async fn do_tx(&mut self, _timeout_in_ms: u32) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;

        self.write_register(Register::RegOpMode, LoRaMode::Tx.value(), false)
            .await
    }

    async fn do_rx(
        &mut self,
        _rx_pkt_params: &PacketParams,
        duty_cycle_params: Option<&DutyCycleParams>,
        rx_continuous: bool,
        rx_boosted_if_supported: bool,
        symbol_timeout: u16,
        _rx_timeout_in_ms: u32,
    ) -> Result<(), RadioError> {
        match duty_cycle_params {
            Some(&_duty_cycle) => return Err(RadioError::DutyCycleUnsupported),
            None => (),
        }

        self.intf.iv.enable_rf_switch_rx().await?;

        let mut symbol_timeout_final = symbol_timeout;
        if rx_continuous {
            symbol_timeout_final = 0;
        }
        if symbol_timeout_final > 0x00ffu16 {
            return Err(RadioError::InvalidSymbolTimeout);
        }
        self.set_lora_symbol_num_timeout(symbol_timeout_final as u8).await?;

        let mut lna_gain_final = LnaGain::G1.value();
        if rx_boosted_if_supported {
            lna_gain_final = LnaGain::G1.boosted_value();
        }
        self.write_register(Register::RegLna, lna_gain_final, false).await?;

        self.write_register(Register::RegFifoAddrPtr, 0x00u8, false).await?;
        self.write_register(Register::RegPayloadLength, 0xffu8, false).await?; // reset payload length (from original implementation)

        if rx_continuous {
            self.write_register(Register::RegOpMode, LoRaMode::RxContinuous.value(), false)
                .await
        } else {
            self.write_register(Register::RegOpMode, LoRaMode::RxSingle.value(), false)
                .await
        }
    }

    async fn get_rx_payload(
        &mut self,
        _rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<u8, RadioError> {
        let payload_length = self.read_register(Register::RegRxNbBytes).await?;
        if (payload_length as usize) > receiving_buffer.len() {
            return Err(RadioError::PayloadSizeMismatch(
                payload_length as usize,
                receiving_buffer.len(),
            ));
        }
        let fifo_addr = self.read_register(Register::RegFifoRxCurrentAddr).await?;
        self.write_register(Register::RegFifoAddrPtr, fifo_addr, false).await?;
        for i in 0..payload_length {
            let byte = self.read_register(Register::RegFifo).await?;
            receiving_buffer[i as usize] = byte;
        }
        self.write_register(Register::RegFifoAddrPtr, 0x00u8, false).await?;

        Ok(payload_length)
    }

    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError> {
        let rssi_raw = self.read_register(Register::RegPktRssiValue).await?;
        let rssi = (rssi_raw as i16) - 157i16; // or -164 for low frequency port ???
        let snr_raw = self.read_register(Register::RegPktRssiValue).await?;
        let snr = snr_raw as i16;
        Ok(PacketStatus { rssi, snr })
    }

    async fn do_cad(
        &mut self,
        _mdltn_params: &ModulationParams,
        rx_boosted_if_supported: bool,
    ) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;

        let mut lna_gain_final = LnaGain::G1.value();
        if rx_boosted_if_supported {
            lna_gain_final = LnaGain::G1.boosted_value();
        }
        self.write_register(Register::RegLna, lna_gain_final, false).await?;

        self.write_register(Register::RegOpMode, LoRaMode::Cad.value(), false)
            .await
    }

    // Set the IRQ mask to disable unwanted interrupts, enable interrupts on DIO0 (the IRQ pin), and allow interrupts.
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError> {
        match radio_mode {
            Some(RadioMode::Transmit) => {
                self.write_register(
                    Register::RegIrqFlagsMask,
                    IrqMask::All.value() ^ IrqMask::TxDone.value(),
                    false,
                )
                .await?;

                let mut dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                dio_mapping_1 = (dio_mapping_1 & DioMapping1Dio0::Mask.value()) | DioMapping1Dio0::TxDone.value();
                self.write_register(Register::RegDioMapping1, dio_mapping_1, false)
                    .await?;

                self.write_register(Register::RegIrqFlags, 0x00u8, false).await?;
            }
            Some(RadioMode::Receive) => {
                self.write_register(
                    Register::RegIrqFlagsMask,
                    IrqMask::All.value()
                        ^ (IrqMask::RxDone.value() | IrqMask::RxTimeout.value() | IrqMask::CRCError.value()),
                    false,
                )
                .await?;

                let mut dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                dio_mapping_1 = (dio_mapping_1 & DioMapping1Dio0::Mask.value()) | DioMapping1Dio0::RxDone.value();
                self.write_register(Register::RegDioMapping1, dio_mapping_1, false)
                    .await?;

                self.write_register(Register::RegIrqFlags, 0x00u8, false).await?;
            }
            Some(RadioMode::ChannelActivityDetection) => {
                self.write_register(
                    Register::RegIrqFlagsMask,
                    IrqMask::All.value() ^ (IrqMask::CADDone.value() | IrqMask::CADActivityDetected.value()),
                    false,
                )
                .await?;

                let mut dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                dio_mapping_1 = (dio_mapping_1 & DioMapping1Dio0::Mask.value()) | DioMapping1Dio0::CadDone.value();
                self.write_register(Register::RegDioMapping1, dio_mapping_1, false)
                    .await?;

                self.write_register(Register::RegIrqFlags, 0x00u8, false).await?;
            }
            _ => {
                self.write_register(Register::RegIrqFlagsMask, IrqMask::All.value(), false)
                    .await?;

                let mut dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                dio_mapping_1 = (dio_mapping_1 & DioMapping1Dio0::Mask.value()) | DioMapping1Dio0::Other.value();
                self.write_register(Register::RegDioMapping1, dio_mapping_1, false)
                    .await?;

                self.write_register(Register::RegIrqFlags, 0xffu8, false).await?;
            }
        }

        Ok(())
    }

    /// Process the radio irq
    async fn process_irq(
        &mut self,
        radio_mode: RadioMode,
        _rx_continuous: bool,
        cad_activity_detected: Option<&mut bool>,
    ) -> Result<(), RadioError> {
        loop {
            debug!("process_irq loop entered");

            self.intf.iv.await_irq().await?;

            let irq_flags = self.read_register(Register::RegIrqFlags).await?;
            self.write_register(Register::RegIrqFlags, 0xffu8, false).await?; // clear all interrupts

            debug!("process_irq satisfied: irq_flags = 0x{:x}", irq_flags);

            // check for errors and unexpected interrupt masks (based on radio mode)
            if (irq_flags & IrqMask::CRCError.value()) == IrqMask::CRCError.value() {
                if radio_mode == RadioMode::Receive {
                    return Err(RadioError::CRCErrorOnReceive);
                } else {
                    return Err(RadioError::CRCErrorUnexpected);
                }
            } else if (irq_flags & IrqMask::RxTimeout.value()) == IrqMask::RxTimeout.value() {
                if radio_mode == RadioMode::Receive {
                    return Err(RadioError::ReceiveTimeout);
                } else {
                    return Err(RadioError::TimeoutUnexpected);
                }
            } else if ((irq_flags & IrqMask::TxDone.value()) == IrqMask::TxDone.value())
                && (radio_mode != RadioMode::Transmit)
            {
                return Err(RadioError::TransmitDoneUnexpected);
            } else if ((irq_flags & IrqMask::RxDone.value()) == IrqMask::RxDone.value())
                && (radio_mode != RadioMode::Receive)
            {
                return Err(RadioError::ReceiveDoneUnexpected);
            } else if (((irq_flags & IrqMask::CADActivityDetected.value()) == IrqMask::CADActivityDetected.value())
                || ((irq_flags & IrqMask::CADDone.value()) == IrqMask::CADDone.value()))
                && (radio_mode != RadioMode::ChannelActivityDetection)
            {
                return Err(RadioError::CADUnexpected);
            }

            if (irq_flags & IrqMask::HeaderValid.value()) == IrqMask::HeaderValid.value() {
                debug!("HeaderValid");
            }

            // handle completions
            if (irq_flags & IrqMask::TxDone.value()) == IrqMask::TxDone.value() {
                return Ok(());
            } else if (irq_flags & IrqMask::RxDone.value()) == IrqMask::RxDone.value() {
                return Ok(());
            } else if (irq_flags & IrqMask::CADDone.value()) == IrqMask::CADDone.value() {
                if cad_activity_detected.is_some() {
                    *(cad_activity_detected.unwrap()) =
                        (irq_flags & IrqMask::CADActivityDetected.value()) == IrqMask::CADActivityDetected.value();
                }
                return Ok(());
            }

            // if an interrupt occurred for other than an error or operation completion (currently, only HeaderValid is in that category), loop to wait again
        }
    }
}
