mod radio_kind_params;

use defmt::info;
use crate::{mod_params::*, mod_params::RadioError::*, Interface, InterfaceVariant, RadioKind};
use radio_kind_params::*;
use embedded_hal_async::{spi::*, delay::DelayUs};

// Syncword for public and private networks
const LORA_MAC_SYNCWORD: u8 = 0x34;

// TCXO flag
const TCXO_FOR_OSCILLATOR: u8 = 0x10u8;

// Frequency synthesizer step for frequency calculation (Hz)
const FREQUENCY_SYNTHESIZER_STEP: f64 = 61.03515625;  // FXOSC (32 MHz) * 1000000 (Hz/MHz) / 524288 (2^19)

// Number of symbols for symbol detection timeout
const LORA_SYMB_NUM_TIMEOUT: u8 = 0x05;

// Possible LoRa bandwidths
const LORA_BANDWIDTHS: [Bandwidth; 3] =
    [Bandwidth::_125KHz, Bandwidth::_250KHz, Bandwidth::_500KHz];

impl ModulationParams {
    pub fn new_for_sx1276_7_8_9(
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate) -> Result<Self, RadioError> {
        // low date rate optimize rule ???
        Ok(Self { spreading_factor, bandwidth, coding_rate, low_data_rate_optimize: 0})
    }
}

impl PacketParams {
    pub fn new_for_sx1276_7_8_9(
        mut preamble_length: u16,
        implicit_header: bool,
        payload_length: u8,
        crc_on: bool,
        iq_inverted: bool, modulation_params: ModulationParams) -> Result<Self, RadioError> {
        // preamble length rule ???
            
        Ok(Self { preamble_length, implicit_header, payload_length, crc_on, iq_inverted })
    }
}

pub struct SX1276_7_8_9<SPI, IV> {
    radio_type: RadioType,
    intf: Interface<SPI, IV>,
}

impl<SPI, IV> SX1276_7_8_9<SPI, IV>
where
    SPI: SpiBus<u8> + 'static,
    IV: InterfaceVariant + 'static,
{
    pub fn new(radio_type: RadioType, spi: SPI, iv: IV) -> Self {
        let intf = Interface::new(spi, iv);
        Self { radio_type, intf }
    }

    // Utility functions

    // Set the number of symbols the radio will wait to validate a reception
    async fn set_lora_symbol_num_timeout(&mut self, symbol_num: u8) -> Result<(), RadioError> {
        let write_buffer = [Register::RegSymbTimeoutLsb.write_addr(), symbol_num];
        self.intf.write(&[&write_buffer], false).await
    }

    // Set the over current protection (mA) on the radio
    async fn set_ocp(&mut self, ma: u8) -> Result<(), RadioError> {
        let mut ocp_trim: u8 = 27;

        if ma <= 120 {
            ocp_trim = (ma - 45) / 5;
        } else if ma <= 240 {
            ocp_trim = (ma + 30) / 10;
        }
        let write_buffer = [Register::RegOcp.write_addr(), 0x20 | (0x1f & ocp_trim)];  // check this ???
        self.intf.write(&[&write_buffer], false).await
    }
}

impl<SPI, IV> RadioKind for SX1276_7_8_9<SPI, IV>
where
    SPI: SpiBus<u8> + 'static,
    IV: InterfaceVariant + 'static,
{
    fn get_radio_type(&mut self) -> RadioType {
        self.radio_type
    }

    async fn reset(&mut self, delay: &mut impl DelayUs) -> Result<(), RadioError> {
        self.intf.iv.reset(delay).await?;
        self.set_sleep(delay).await?;  // ensure sleep mode is entered so that the LoRa mode bit is set
        Ok(())
    }

    async fn ensure_ready(&mut self, mode: RadioMode) -> Result<(), RadioError> {
        Ok(())
    }

    // Use DIO2 to control an RF Switch
    async fn init_rf_switch(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_standby(&mut self) -> Result<(), RadioError> {
        let write_buffer = [Register::RegOpMode.write_addr(), LoRaMode::Standby.value()];
        self.intf.write(&[&write_buffer], false).await?;
        self.intf.iv.disable_rf_switch().await
    }

    async fn set_sleep(&mut self, delay: &mut impl DelayUs) -> Result<bool, RadioError> {
        self.intf.iv.disable_rf_switch().await?;
        let write_buffer = [Register::RegOpMode.write_addr(), LoRaMode::Sleep.value()];
        self.intf.write(&[&write_buffer], true).await?;

        Ok(false)  // warm start unavailable for sx127x ???
    }

    /// The sx127x LoRa mode is set when setting a mode while in sleep mode.  Only one type of sync word is supported.
    async fn set_lora_modem(&mut self, enable_public_network: bool) -> Result<(), RadioError> {
        let write_buffer = [Register::RegSyncWord.write_addr(), LORA_MAC_SYNCWORD];
        self.intf.write(&[&write_buffer], false).await
    }

    async fn set_oscillator(&mut self) -> Result<(), RadioError> {
        let write_buffer = [Register::RegTcxo.write_addr(), TCXO_FOR_OSCILLATOR];
        self.intf.write(&[&write_buffer], false).await
    }

    async fn set_regulator_mode(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_tx_rx_buffer_base_address(&mut self, tx_base_addr: usize, rx_base_addr: usize) -> Result<(), RadioError> {
        if tx_base_addr > 255 || rx_base_addr > 255 {
            return Err(RadioError::InvalidBaseAddress(tx_base_addr, rx_base_addr));
        }
        let mut write_buffer = [Register::RegFifoTxBaseAddr.write_addr(), 0];
        self.intf.write(&[&write_buffer], false).await?;
        write_buffer = [Register::RegFifoRxBaseAddr.write_addr(), 0];
        self.intf.write(&[&write_buffer], false).await
    }

    //   power        RF output power (dBm)
    //   is_tx_prep   indicates which ramp up time to use
    async fn set_tx_power_and_ramp_time(&mut self, mut power: i8, tx_boosted_if_possible: bool, is_tx_prep: bool) -> Result<(), RadioError> {
        let mut write_buffer = [00u8; 2];

        // Fix magic numbers and check algorithm ???
        if tx_boosted_if_possible {
            if power > 17 {
                if power > 20 {
                    power = 20;
                }
                // subtract 3 from power, so 18 - 20 maps to 15 - 17
                power -= 3;

                // High Power +20 dBm Operation (Semtech SX1276/77/78/79 5.4.3.)
                write_buffer = [Register::RegPaDac.write_addr(), 0x87];
                self.intf.write(&[&write_buffer], false).await?;
                self.set_ocp(140).await?;
            } else {
                if power < 2 {
                    power = 2;
                }
                //Default value PA_HF/LF or +17dBm
                write_buffer = [Register::RegPaDac.write_addr(), 0x84];
                self.intf.write(&[&write_buffer], false).await?;
                self.set_ocp(100).await?;
            }
            power -= 2;  // does this account for the power -= 3 above ???
            write_buffer = [Register::RegPaConfig.write_addr(), PaConfig::PaBoost.value() | power as u8];
            self.intf.write(&[&write_buffer], false).await?;
        } else {
            // RFO
            if power < 0 {
                power = 0;
            } else if power > 14 {
                power = 14;
            }

            // no DAC or OCP setting ???
            write_buffer = [Register::RegPaConfig.write_addr(), (0x70 | power) as u8];
            self.intf.write(&[&write_buffer], false).await?;      
        }

        let ramp_time = match is_tx_prep {
            true => RampTime::Ramp40Us,    // for instance, prior to TX or CAD
            false => RampTime::Ramp250Us,  // for instance, on initialization
        };
        write_buffer = [Register::RegPaRamp.write_addr(), ramp_time.value()];
        self.intf.write(&[&write_buffer], false).await

        
    }

    async fn update_retention_list(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_modulation_params(&mut self, mod_params: ModulationParams) -> Result<(), RadioError> {
        let spreading_factor_val = spreading_factor_value(mod_params.spreading_factor)?;
        let bandwidth_val = bandwidth_value(mod_params.bandwidth)?;
        let coding_rate_val = coding_rate_value(mod_params.coding_rate)?;
        /*
        let op_code_and_mod_params = [
            OpCode::SetModulationParams.value(),
            spreading_factor_val,
            bandwidth_val,
            coding_rate_val,
            mod_params.low_data_rate_optimize
            ];
        self.intf.write(&[&op_code_and_mod_params], false).await?;

        // Handle modulation quality with the 500 kHz LoRa bandwidth (see DS_SX1261-2_V1.2 datasheet chapter 15.1)
        let mut tx_mod = [0x00u8];
        self.intf.read(&[&[OpCode::ReadRegister.value(), Register::TxModulation.addr1(), Register::TxModulation.addr2(), 0x00u8]], &mut tx_mod, None).await?;
        if mod_params.bandwidth == Bandwidth::_500KHz {
            let register_and_tx_mod_update = [
                OpCode::WriteRegister.value(),
                Register::TxModulation.addr1(), Register::TxModulation.addr2(),
                tx_mod[0] & (!(1 << 2))
                ];
            self.intf.write(&[&register_and_tx_mod_update], false).await
        } else {
            let register_and_tx_mod_update = [
                OpCode::WriteRegister.value(),
                Register::TxModulation.addr1(), Register::TxModulation.addr2(),
                tx_mod[0] | (1 << 2)
                ];
            self.intf.write(&[&register_and_tx_mod_update], false).await
        }
        */
        Ok(())
    }

    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError> {
        /*
        let op_code_and_pkt_params = [
            OpCode::SetPacketParams.value(),
            ((pkt_params.preamble_length >> 8) & 0xFF) as u8,
            (pkt_params.preamble_length & 0xFF) as u8,
            pkt_params.implicit_header as u8,
            pkt_params.payload_length,
            pkt_params.crc_on as u8,
            pkt_params.iq_inverted as u8
            ];
        self.intf.write(&[&op_code_and_pkt_params], false).await
        */
        Ok(())
    }

    // Calibrate the image rejection based on the given frequency
    async fn calibrate_image(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        /*
        let mut cal_freq = [0x00u8, 0x00u8];

        if frequency_in_hz > 900000000 {
            cal_freq[0] = 0xE1;
            cal_freq[1] = 0xE9;
        } else if frequency_in_hz > 850000000 {
            cal_freq[0] = 0xD7;
            cal_freq[1] = 0xDB;
        } else if frequency_in_hz > 770000000 {
            cal_freq[0] = 0xC1;
            cal_freq[1] = 0xC5;
        } else if frequency_in_hz > 460000000 {
            cal_freq[0] = 0x75;
            cal_freq[1] = 0x81;
        } else if frequency_in_hz > 425000000 {
            cal_freq[0] = 0x6B;
            cal_freq[1] = 0x6F;
        }

        let op_code_and_cal_freq = [OpCode::CalibrateImage.value(), cal_freq[0], cal_freq[1]];
        self.intf.write(&[&op_code_and_cal_freq], false).await
        */
        Ok(())
    }
    
    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        let frf = (frequency_in_hz as f64 / FREQUENCY_SYNTHESIZER_STEP) as u32;
        let mut write_buffer = [Register::RegFrfMsb.write_addr(), ((frf & 0x00FF0000) >> 16) as u8];
        self.intf.write(&[&write_buffer], false).await?;
        write_buffer = [Register::RegFrfMid.write_addr(), ((frf & 0x0000FF00) >> 8) as u8];
        self.intf.write(&[&write_buffer], false).await?;
        write_buffer = [Register::RegFrfLsb.write_addr(), (frf & 0x000000FF) as u8];
        self.intf.write(&[&write_buffer], false).await
    }

    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        /*
        let op_code_and_offset = [OpCode::WriteBuffer.value(), 0x00u8];
        self.intf.write(&[&op_code_and_offset, payload], false).await
        */
        Ok(())
    }

    async fn do_tx(&mut self, timeout_in_ms: u32) -> Result<(), RadioError> {
        /*
        self.intf.iv.enable_rf_switch_tx().await?;

        let op_code_and_timeout = [
            OpCode::SetTx.value(),
            Self::timeout_1(timeout_in_ms),
            Self::timeout_2(timeout_in_ms),
            Self::timeout_3(timeout_in_ms)];
        self.intf.write(&[&op_code_and_timeout], false).await
        */
        Ok(())
    }

    async fn do_rx(&mut self, rx_pkt_params: &PacketParams, rx_continuous: bool, rx_boosted_if_supported: bool, symbol_timeout: u16, rx_timeout_in_ms: u32) -> Result<(), RadioError> {
        let mut symbol_timeout_final = symbol_timeout;
        let mut rx_timeout_in_ms_final = rx_timeout_in_ms << 6;
        if rx_continuous {
            symbol_timeout_final = 0;
            rx_timeout_in_ms_final = 0;
        }

        let mut lna_gain_final = LnaGain::G1.value();
        if rx_boosted_if_supported {
            lna_gain_final = LnaGain::G1.boosted_value();
        }
        
        /*
        self.intf.iv.enable_rf_switch_rx().await?;

        // stop the Rx timer on header/syncword detection rather than preamble detection
        let op_code_and_false_flag = [OpCode::SetStopRxTimerOnPreamble.value(), 0x00u8];
        self.intf.write(&[&op_code_and_false_flag], false).await?;

        self.set_lora_symbol_num_timeout(LORA_SYMB_NUM_TIMEOUT).await?;

        // Optimize the Inverted IQ Operation (see DS_SX1261-2_V1.2 datasheet chapter 15.4)
        let mut iq_polarity = [0x00u8];
        self.intf.read(&[&[OpCode::ReadRegister.value(), Register::IQPolarity.addr1(), Register::IQPolarity.addr2(), 0x00u8]], &mut iq_polarity, None).await?;
        if rx_pkt_params.iq_inverted {
            let register_and_iq_polarity = [
                OpCode::WriteRegister.value(),
                Register::IQPolarity.addr1(), Register::IQPolarity.addr2(),
                iq_polarity[0] & (!(1 << 2))
                ];
            self.intf.write(&[&register_and_iq_polarity], false).await?;
        } else {
            let register_and_iq_polarity = [
                OpCode::WriteRegister.value(),
                Register::IQPolarity.addr1(), Register::IQPolarity.addr2(),
                iq_polarity[0] | (1 << 2)
                ];
            self.intf.write(&[&register_and_iq_polarity], false).await?;
        }

        let mut write_buffer = [Register::RegLna.write_addr(), lna_gain_final];
        self.intf.write(&[&write_buffer], false).await?;

        let op_code_and_timeout = [
            OpCode::SetRx.value(),
            Self::timeout_1(rx_timeout_in_ms_final),
            Self::timeout_2(rx_timeout_in_ms_final),
            Self::timeout_3(rx_timeout_in_ms_final)];
        self.intf.write(&[&op_code_and_timeout], false).await
        */
        Ok(())
    }

    async fn get_rx_payload(&mut self, rx_pkt_params: &PacketParams, receiving_buffer: &mut [u8]) -> Result<u8, RadioError> {
        /*
        let op_code = [OpCode::GetRxBufferStatus.value()];
        let mut rx_buffer_status = [0x00u8; 2];
        self.intf.read_with_status(&[&op_code], &mut rx_buffer_status).await?;  // handle return status ???

        let mut payload_length_buffer = [0x00u8];
        if rx_pkt_params.implicit_header {
            self.intf.read(&[&[OpCode::ReadRegister.value(), Register::PayloadLength.addr1(), Register::PayloadLength.addr2(), 0x00u8]], &mut payload_length_buffer, None).await?;
        } else {
            payload_length_buffer[0] = rx_buffer_status[0];
        }

        let payload_length = payload_length_buffer[0];
        let offset = rx_buffer_status[1];

        if (payload_length as usize) > receiving_buffer.len() {
            Err(RadioError::PayloadSizeMismatch(payload_length as usize, receiving_buffer.len()))
        } else {
            self.intf.read(&[&[OpCode::ReadBuffer.value(), offset, 0x00u8]], receiving_buffer, Some(payload_length)).await?;
            Ok(payload_length)
        }
        */
        Ok((0))
    }

    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError> {
        /*
        let op_code = [OpCode::GetPacketStatus.value()];
        let mut pkt_status = [0x00u8; 3];
        self.intf.read_with_status(&[&op_code], &mut pkt_status).await?;  // handle return status ???

        // check this ???
        let rssi = ((-(pkt_status[0] as i32)) >> 1) as i8;
        let snr = ((pkt_status[1] as i8) + 2) >> 2;
        let _signal_rssi = ((-(pkt_status[2] as i32)) >> 1) as i8;  // unused currently

        Ok(PacketStatus {
            rssi,
            snr,
        })
        */
        Ok(PacketStatus {
            rssi: 0,
            snr: 0,
        })
    }

    // Set the IRQ mask and DIO masks
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError> {
        /*
        let mut irq_mask: u16 = IrqMask::None.value();
        let mut dio1_mask: u16 = IrqMask::None.value();
        let dio2_mask: u16 = IrqMask::None.value();
        let dio3_mask: u16 = IrqMask::None.value();

        match radio_mode {
            Some(RadioMode::Standby) => {
                irq_mask = IrqMask::All.value();
                dio1_mask = IrqMask::All.value();
            }
            Some(RadioMode::Transmit) => {
                irq_mask = IrqMask::TxDone.value() | IrqMask::RxTxTimeout.value();
                dio1_mask = IrqMask::TxDone.value() | IrqMask::RxTxTimeout.value();
            }
            Some(RadioMode::Receive) | Some(RadioMode::ReceiveDutyCycle) => {
                irq_mask = IrqMask::All.value();
                dio1_mask = IrqMask::All.value();
            }
            Some(RadioMode::ChannelActivityDetection) => {
                irq_mask = IrqMask::CADDone.value() | IrqMask::CADActivityDetected.value();
                dio1_mask = IrqMask::CADDone.value() | IrqMask::CADActivityDetected.value();
            }
            _ => {}
        }
        
        let op_code_and_masks = [
            OpCode::CfgDIOIrq.value(),
            ((irq_mask >> 8) & 0x00FF) as u8,
            (irq_mask & 0x00FF) as u8,
            ((dio1_mask >> 8) & 0x00FF) as u8,
            (dio1_mask & 0x00FF) as u8,
            ((dio2_mask >> 8) & 0x00FF) as u8,
            (dio2_mask & 0x00FF) as u8,
            ((dio3_mask >> 8) & 0x00FF) as u8,
            (dio3_mask & 0x00FF) as u8
            ];
        self.intf.write(&[&op_code_and_masks], false).await
        */
        Ok(())
    }

    /// Process the radio irq
    async fn process_irq(&mut self, radio_mode: RadioMode, rx_continuous: bool) -> Result<(), RadioError> {
        /*
        loop {
            info!("process_irq loop entered");

            /* ???
            let de = self.sub_get_device_errors().await?;
            info!("device_errors: rc_64khz_calibration = {}, rc_13mhz_calibration = {}, pll_calibration = {}, adc_calibration = {}, image_calibration = {}, xosc_start = {}, pll_lock = {}, pa_ramp = {}",
                               de.rc_64khz_calibration, de.rc_13mhz_calibration, de.pll_calibration, de.adc_calibration, de.image_calibration, de.xosc_start, de.pll_lock, de.pa_ramp);
            let st = self.sub_get_status().await?;
            info!(
                "radio status: cmd_status: {:x}, chip_mode: {:x}",
                st.cmd_status, st.chip_mode
            );
            */

            self.intf.iv.await_irq().await?;
            let op_code = [OpCode::GetIrqStatus.value()];
            let mut irq_status = [0x00u8, 0x00u8];
            self.intf.read_with_status(&[&op_code], &mut irq_status).await?;  // handle return status ???
            let irq_flags = ((irq_status[0] as u16) << 8) | (irq_status[1] as u16);
            let op_code_and_irq_status = [
                OpCode::ClrIrqStatus.value(),
                irq_status[0],
                irq_status[1]];
            self.intf.write(&[&op_code_and_irq_status], false).await?;

            info!("process_irq satisfied: irq_flags = {:x}", irq_flags);

            // check for errors and unexpected interrupt masks (based on radio mode)
            if (irq_flags & IrqMask::HeaderError.value()) == IrqMask::HeaderError.value() {
                return Err(RadioError::HeaderError);
            } else if (irq_flags & IrqMask::CRCError.value()) == IrqMask::CRCError.value() {
                if radio_mode == RadioMode::Receive {
                    return Err(RadioError::CRCErrorOnReceive);
                } else {
                    return Err(RadioError::CRCErrorUnexpected);
                }
            } else if (irq_flags & IrqMask::RxTxTimeout.value()) == IrqMask::RxTxTimeout.value() {
                if radio_mode == RadioMode::Transmit {
                    return Err(RadioError::TransmitTimeout);
                } else if radio_mode == RadioMode::Receive {
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
            } else if (((irq_flags & IrqMask::CADActivityDetected.value())
                == IrqMask::CADActivityDetected.value())
                || ((irq_flags & IrqMask::CADDone.value()) == IrqMask::CADDone.value()))
                && (radio_mode != RadioMode::ChannelActivityDetection)
            {
                return Err(RadioError::CADUnexpected);
            }

            if (irq_flags & IrqMask::HeaderValid.value()) == IrqMask::HeaderValid.value() {
                info!("HeaderValid");
            } else if (irq_flags & IrqMask::PreambleDetected.value())
                == IrqMask::PreambleDetected.value()
            {
                info!("PreambleDetected");
            } else if (irq_flags & IrqMask::SyncwordValid.value()) == IrqMask::SyncwordValid.value()
            {
                info!("SyncwordValid");
            }

            // handle completions
            if (irq_flags & IrqMask::TxDone.value()) == IrqMask::TxDone.value() {
                return Ok(());
            } else if (irq_flags & IrqMask::RxDone.value()) == IrqMask::RxDone.value() {
                if !rx_continuous {
                    // implicit header mode timeout behavior (see DS_SX1261-2_V1.2 datasheet chapter 15.3)
                    let register_and_clear = [OpCode::WriteRegister.value(), Register::RTCCtrl.addr1(), Register::RTCCtrl.addr2(), 0x00u8];
                    self.intf.write(&[&register_and_clear], false).await?;

                    let mut evt_clr = [0x00u8];
                    self.intf.read(&[&[OpCode::ReadRegister.value(), Register::EvtClr.addr1(), Register::EvtClr.addr2(), 0x00u8]], &mut evt_clr, None).await?;
                    evt_clr[0] |= 1 << 1;
                    let register_and_evt_clear = [OpCode::WriteRegister.value(), Register::EvtClr.addr1(), Register::EvtClr.addr2(), evt_clr[0]];
                    self.intf.write(&[&register_and_evt_clear], false).await?;
                }
                return Ok(());
            } else if (irq_flags & IrqMask::CADDone.value()) == IrqMask::CADDone.value() {
                /*
                if cad_activity_detected.is_some() {
                    *(cad_activity_detected.unwrap()) = (irq_flags
                        & IrqMask::CADActivityDetected.value())
                        == IrqMask::CADActivityDetected.value();
                }
                */
                return Ok(());
            }

            // if an interrupt occurred for other than an error or operation completion (currently, PreambleDetected, SyncwordValid, and HeaderValid
            // are in that category), loop to wait again
        }
        */
        Ok(())
    }
}
