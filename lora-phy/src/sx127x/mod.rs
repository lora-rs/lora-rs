mod radio_kind_params;
mod sx1272;
pub use sx1272::Sx1272;
mod sx1276;
pub use sx1276::Sx1276;

use defmt::debug;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::*;
use radio_kind_params::*;

use crate::mod_params::*;
use crate::mod_traits::IrqState;
use crate::{InterfaceVariant, RadioKind, SpiInterface};

// Syncwords for public and private networks
const LORA_MAC_PUBLIC_SYNCWORD: u8 = 0x34; // corresponds to sx126x 0x3444
const LORA_MAC_PRIVATE_SYNCWORD: u8 = 0x12; // corresponds to sx126x 0x1424

// TCXO flag
const TCXO_FOR_OSCILLATOR: u8 = 0x10u8;

// Frequency synthesizer step for frequency calculation (Hz)
const FREQUENCY_SYNTHESIZER_STEP: f64 = 61.03515625; // FXOSC (32 MHz) * 1000000 (Hz/MHz) / 524288 (2^19)

// Limits for preamble detection window in single reception mode
const SX127X_MIN_LORA_SYMB_NUM_TIMEOUT: u16 = 4;
const SX127X_MAX_LORA_SYMB_NUM_TIMEOUT: u16 = 1023;

// Constant values need to compute the RSSI value
const SX1272_RSSI_OFFSET: i16 = -139;
const SX1276_RSSI_OFFSET_LF: i16 = -164;
const SX1276_RSSI_OFFSET_HF: i16 = -157;
const SX1276_RF_MID_BAND_THRESH: u32 = 525_000_000;

/// Configuration for SX127x-based boards
pub struct Config<C: Sx127xVariant> {
    /// LoRa chip used on specific board
    pub chip: C,
    /// Whether board is using crystal oscillator or external clock
    pub tcxo_used: bool,
    /// Whether to use PA_BOOST for transmit instead of RFO (sx1272) or RFO_LF (sx1276).
    /// NB! Depends on board layout.
    pub tx_boost: bool,
    /// Whether to boost receive
    pub rx_boost: bool,
}

/// Base for the RadioKind implementation for the LoRa chip kind and board type
pub struct Sx127x<SPI, IV, C: Sx127xVariant + Sized> {
    intf: SpiInterface<SPI, IV>,
    config: Config<C>,
}

impl<SPI, IV, C> Sx127x<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Sx127xVariant,
{
    /// Create an instance of the RadioKind implementation for the LoRa chip kind and board type
    pub fn new(spi: SPI, iv: IV, config: Config<C>) -> Self {
        let intf = SpiInterface::new(spi, iv);
        Self { intf, config }
    }

    // Utility functions
    async fn write_register(&mut self, register: Register, value: u8) -> Result<(), RadioError> {
        let write_buffer = [register.write_addr(), value];
        self.intf.write(&write_buffer, false).await
    }

    async fn read_register(&mut self, register: Register) -> Result<u8, RadioError> {
        let write_buffer = [register.read_addr()];
        let mut read_buffer = [0x00u8];
        self.intf.read(&write_buffer, &mut read_buffer).await?;
        Ok(read_buffer[0])
    }

    async fn read_buffer(&mut self, register: Register, buf: &mut [u8]) -> Result<(), RadioError> {
        self.intf.read(&[register.read_addr()], buf).await
    }

    async fn write_buffer(&mut self, register: Register, buf: &[u8]) -> Result<(), RadioError> {
        self.intf.write_with_payload(&[register.write_addr()], buf, false).await
    }

    // Set the number of symbols the radio will wait to detect a reception (up to 1023 symbols)
    async fn set_lora_symbol_num_timeout(&mut self, symbol_num: u16) -> Result<(), RadioError> {
        let val = symbol_num.min(SX127X_MAX_LORA_SYMB_NUM_TIMEOUT);

        let symbol_num_msb = ((val >> 8) & 0x03) as u8;
        let symbol_num_lsb = (val & 0xff) as u8;
        let mut config_2 = self.read_register(Register::RegModemConfig2).await?;
        config_2 = (config_2 & 0xfcu8) | symbol_num_msb;
        self.write_register(Register::RegModemConfig2, config_2).await?;
        self.write_register(Register::RegSymbTimeoutLsb, symbol_num_lsb).await
    }

    // Set the over current protection (mA) on the radio
    async fn set_ocp(&mut self, ocp_trim: OcpTrim) -> Result<(), RadioError> {
        self.write_register(Register::RegOcp, ocp_trim.value()).await
    }
}

impl<SPI, IV, C> RadioKind for Sx127x<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Sx127xVariant,
{
    async fn init_lora(&mut self, is_public_network: bool) -> Result<(), RadioError> {
        if self.config.tcxo_used {
            self.write_register(C::reg_txco(), TCXO_FOR_OSCILLATOR).await?;
        }

        let syncword = if is_public_network {
            LORA_MAC_PUBLIC_SYNCWORD
        } else {
            LORA_MAC_PRIVATE_SYNCWORD
        };
        self.write_register(Register::RegSyncWord, syncword).await?;

        self.set_tx_rx_buffer_base_address(0, 0).await?;
        Ok(())
    }

    fn create_modulation_params(
        &self,
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        frequency_in_hz: u32,
    ) -> Result<ModulationParams, RadioError> {
        // Parameter validation
        spreading_factor_value(spreading_factor)?;
        coding_rate_value(coding_rate)?;
        C::bandwidth_value(bandwidth)?;
        if ((bandwidth == Bandwidth::_250KHz) || (bandwidth == Bandwidth::_500KHz)) && (frequency_in_hz < 400_000_000) {
            return Err(RadioError::InvalidBandwidthForFrequency);
        }

        // Section 4.1.1.5 and 4.1.1.6
        let bw_in_hz = u32::from(bandwidth);
        let symbol_duration = 1000 / (bw_in_hz / (0x01u32 << spreading_factor_value(spreading_factor)?));
        let mut low_data_rate_optimize = 0x00u8;
        if symbol_duration > 16 {
            low_data_rate_optimize = 0x01u8
        }

        Ok(ModulationParams {
            spreading_factor,
            bandwidth,
            coding_rate,
            low_data_rate_optimize,
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
        modulation_params: &ModulationParams,
    ) -> Result<PacketParams, RadioError> {
        // Parameter validation
        if (modulation_params.spreading_factor == SpreadingFactor::_6) && !implicit_header {
            return Err(RadioError::InvalidSF6ExplicitHeaderRequest);
        }

        Ok(PacketParams {
            preamble_length,
            implicit_header,
            payload_length,
            crc_on,
            iq_inverted,
        })
    }

    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        self.intf.iv.reset(delay).await?;
        self.set_sleep(false, delay).await?; // ensure sleep mode is entered so that the LoRa mode bit is set
        Ok(())
    }

    async fn ensure_ready(&mut self, _mode: RadioMode) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_standby(&mut self) -> Result<(), RadioError> {
        self.write_register(Register::RegOpMode, LoRaMode::Standby.value())
            .await?;
        self.intf.iv.disable_rf_switch().await
    }

    async fn set_sleep(&mut self, _warm_start_if_possible: bool, _delay: &mut impl DelayNs) -> Result<(), RadioError> {
        // Warm start is unavailable for sx127x
        self.intf.iv.disable_rf_switch().await?;
        let buf = [Register::RegOpMode.write_addr(), LoRaMode::Sleep.value()];
        // NB! Switching to sleep mode is "sleep" command...
        self.intf.write(&buf, true).await?;

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
        self.write_register(Register::RegFifoTxBaseAddr, 0x00u8).await?;
        self.write_register(Register::RegFifoRxBaseAddr, 0x00u8).await
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
        is_tx_prep: bool,
    ) -> Result<(), RadioError> {
        debug!("tx power = {}", p_out);

        // Configure tx power and boost
        C::set_tx_power(self, p_out, self.config.tx_boost).await?;

        let ramp_time = match is_tx_prep {
            true => RampTime::Ramp40Us,   // for instance, prior to TX or CAD
            false => RampTime::Ramp250Us, // for instance, on initialization
        };

        let val = C::ramp_value(ramp_time);
        self.write_register(Register::RegPaRamp, val).await
    }

    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        let sf_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let bw_val = C::bandwidth_value(mdltn_params.bandwidth)?;
        let coding_rate_denominator_val = coding_rate_denominator_value(mdltn_params.coding_rate)?;
        debug!(
            "sf = {}, bw = {}, cr_denom = {}",
            sf_val, bw_val, coding_rate_denominator_val
        );
        // Configure LoRa optimization (0x31) and detection threshold registers (0x37)
        let (opt, thr) = match mdltn_params.spreading_factor {
            SpreadingFactor::_6 => (0x05, 0x0c),
            _ => (0x03, 0x0a),
        };
        let reg_val = self.read_register(Register::RegDetectionOptimize).await?;
        // Keep reserved bits [6:3] for RegDetectOptimize
        let val = (reg_val & 0b0111_1000) | opt;
        self.write_register(Register::RegDetectionOptimize, val).await?;
        self.write_register(Register::RegDetectionThreshold, thr).await?;
        // Spreading Factor, Bandwidth, codingrate, ldro

        C::set_modulation_params(self, mdltn_params).await?;

        Ok(())
    }

    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError> {
        self.write_register(
            Register::RegPreambleMsb,
            ((pkt_params.preamble_length >> 8) & 0x00ff) as u8,
        )
        .await?;
        self.write_register(Register::RegPreambleLsb, (pkt_params.preamble_length & 0x00ff) as u8)
            .await?;

        C::set_packet_params(self, pkt_params).await?;

        // IQ inversion:
        // RegInvertiq - [0x33]
        // [6] - InvertIQRX
        // [5:1] - Reserved: 0x13
        // [0] - InvertIQTX
        // RegInvertiq2 - [0x3b]
        // Set to 0x19 when RX, otherwise set 0x1d
        let (iq1, iq2) = match pkt_params.iq_inverted {
            true => (1 << 6, 0x19),
            false => (1 << 0, 0x1d),
        };
        // Keep reserved value for InvertIq as well
        self.write_register(Register::RegInvertiq, (0x13 << 1) | iq1).await?;
        self.write_register(Register::RegInvertiq2, iq2).await?;
        Ok(())
    }

    // Calibrate the image rejection based on the given frequency
    async fn calibrate_image(&mut self, _frequency_in_hz: u32) -> Result<(), RadioError> {
        // An automatic process, but can set bit ImageCalStart in RegImageCal, when the device is in Standby mode.
        Ok(())
    }

    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        debug!("channel = {}", frequency_in_hz);
        let frf = (frequency_in_hz as f64 / FREQUENCY_SYNTHESIZER_STEP) as u32;
        self.write_register(Register::RegFrfMsb, ((frf & 0x00FF0000) >> 16) as u8)
            .await?;
        self.write_register(Register::RegFrfMid, ((frf & 0x0000FF00) >> 8) as u8)
            .await?;
        self.write_register(Register::RegFrfLsb, (frf & 0x000000FF) as u8).await
    }

    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        self.write_register(Register::RegFifoAddrPtr, 0x00u8).await?;
        self.write_register(Register::RegPayloadLength, 0x00u8).await?;
        self.write_buffer(Register::RegFifo, payload).await?;
        self.write_register(Register::RegPayloadLength, payload.len() as u8)
            .await
    }

    async fn do_tx(&mut self) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;

        self.write_register(Register::RegOpMode, LoRaMode::Tx.value()).await
    }

    async fn do_rx(&mut self, rx_mode: RxMode) -> Result<(), RadioError> {
        let (num_symbols, mode) = match rx_mode {
            RxMode::DutyCycle(_) => Err(RadioError::DutyCycleUnsupported),
            RxMode::Single(ns) => Ok((ns.max(SX127X_MIN_LORA_SYMB_NUM_TIMEOUT), LoRaMode::RxSingle)),
            RxMode::Continuous => Ok((0, LoRaMode::RxContinuous)),
        }?;

        self.intf.iv.enable_rf_switch_rx().await?;

        self.set_lora_symbol_num_timeout(num_symbols).await?;

        let lna_gain = if self.config.rx_boost {
            LnaGain::G1.boosted_value()
        } else {
            LnaGain::G1.value()
        };
        self.write_register(Register::RegLna, lna_gain).await?;

        self.write_register(Register::RegFifoAddrPtr, 0x00u8).await?;
        self.write_register(Register::RegPayloadLength, 0xffu8).await?;

        self.write_register(Register::RegOpMode, mode.value()).await
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
        self.write_register(Register::RegFifoAddrPtr, fifo_addr).await?;
        self.read_buffer(Register::RegFifo, &mut receiving_buffer[0..payload_length as usize])
            .await?;
        self.write_register(Register::RegFifoAddrPtr, 0x00u8).await?;

        Ok(payload_length)
    }

    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError> {
        let snr = {
            let packet_snr = self.read_register(Register::RegPktSnrValue).await?;
            packet_snr as i8 as i16 / 4
        };

        let rssi = {
            let packet_rssi = self.read_register(Register::RegPktRssiValue).await?;

            let rssi_offset = C::rssi_offset(self).await?;

            if snr >= 0 {
                rssi_offset + (packet_rssi as f32 * 16.0 / 15.0) as i16
            } else {
                rssi_offset + (packet_rssi as i16) + snr
            }
        };

        Ok(PacketStatus { rssi, snr })
    }

    async fn do_cad(&mut self, _mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;

        let mut lna_gain_final = LnaGain::G1.value();
        if self.config.rx_boost {
            lna_gain_final = LnaGain::G1.boosted_value();
        }
        self.write_register(Register::RegLna, lna_gain_final).await?;

        self.write_register(Register::RegOpMode, LoRaMode::Cad.value()).await
    }

    // Set the IRQ mask to disable unwanted interrupts,
    // enable interrupts on DIO pins (sx127x has multiple),
    // and allow interrupts.
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError> {
        match radio_mode {
            Some(RadioMode::Transmit) => {
                self.write_register(
                    Register::RegIrqFlagsMask,
                    IrqMask::All.value() ^ IrqMask::TxDone.value(),
                )
                .await?;

                let mut dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                dio_mapping_1 = (dio_mapping_1 & DioMapping1Dio0::Mask.value()) | DioMapping1Dio0::TxDone.value();
                self.write_register(Register::RegDioMapping1, dio_mapping_1).await?;

                self.write_register(Register::RegIrqFlags, 0x00u8).await?;
            }
            Some(RadioMode::Receive(_)) => {
                self.write_register(
                    Register::RegIrqFlagsMask,
                    IrqMask::All.value()
                        ^ (IrqMask::RxDone.value()
                            | IrqMask::RxTimeout.value()
                            | IrqMask::CRCError.value()
                            | IrqMask::HeaderValid.value()),
                )
                .await?;

                // HeaderValid and CRCError are mutually exclusive when attempting to
                // trigger DIO-based interrupt, so our approach is to trigger HeaderValid
                // as this is required for preamble detection.
                // TODO: RxTimeout should be configured on DIO1
                let dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                let val = (dio_mapping_1 & DioMapping1Dio0::Mask.value() & DioMapping1Dio3::Mask.value())
                    | (DioMapping1Dio0::RxDone.value() | DioMapping1Dio3::ValidHeader.value());
                self.write_register(Register::RegDioMapping1, val).await?;

                self.write_register(Register::RegIrqFlags, 0x00u8).await?;
            }
            Some(RadioMode::ChannelActivityDetection) => {
                self.write_register(
                    Register::RegIrqFlagsMask,
                    IrqMask::All.value() ^ (IrqMask::CADDone.value() | IrqMask::CADActivityDetected.value()),
                )
                .await?;

                let mut dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                dio_mapping_1 = (dio_mapping_1 & DioMapping1Dio0::Mask.value()) | DioMapping1Dio0::CadDone.value();
                self.write_register(Register::RegDioMapping1, dio_mapping_1).await?;

                self.write_register(Register::RegIrqFlags, 0x00u8).await?;
            }
            _ => {
                self.write_register(Register::RegIrqFlagsMask, IrqMask::All.value())
                    .await?;

                let mut dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                dio_mapping_1 = (dio_mapping_1 & DioMapping1Dio0::Mask.value()) | DioMapping1Dio0::Other.value();
                self.write_register(Register::RegDioMapping1, dio_mapping_1).await?;
                self.write_register(Register::RegIrqFlags, 0xffu8).await?;
            }
        }

        Ok(())
    }

    async fn await_irq(&mut self) -> Result<(), RadioError> {
        self.intf.iv.await_irq().await
    }

    /// Process the radio IRQ. Log unexpected interrupts. Packets from other
    /// devices can cause unexpected interrupts.
    ///
    /// NB! Do not await this future in a select branch as interrupting it
    /// mid-flow could cause radio lock up.
    async fn process_irq_event(
        &mut self,
        radio_mode: RadioMode,
        cad_activity_detected: Option<&mut bool>,
        clear_interrupts: bool,
    ) -> Result<Option<IrqState>, RadioError> {
        let irq_flags = self.read_register(Register::RegIrqFlags).await?;
        if clear_interrupts {
            self.write_register(Register::RegIrqFlags, 0xffu8).await?; // clear all interrupts
        }

        match radio_mode {
            RadioMode::Transmit => {
                if (irq_flags & IrqMask::TxDone.value()) == IrqMask::TxDone.value() {
                    debug!("TxDone in radio mode {}", radio_mode);
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Receive(RxMode::Continuous) | RadioMode::Receive(RxMode::Single(_)) => {
                if (irq_flags & IrqMask::RxDone.value()) == IrqMask::RxDone.value() {
                    debug!("RxDone in radio mode {}", radio_mode);
                    return Ok(Some(IrqState::Done));
                }
                if (irq_flags & IrqMask::RxTimeout.value()) == IrqMask::RxTimeout.value() {
                    debug!("RxTimeout in radio mode {}", radio_mode);
                    return Err(RadioError::ReceiveTimeout);
                }
                if IrqMask::HeaderValid.is_set_in(irq_flags) {
                    debug!("HeaderValid in radio mode {}", radio_mode);
                    return Ok(Some(IrqState::PreambleReceived));
                }
            }
            RadioMode::ChannelActivityDetection => {
                if (irq_flags & IrqMask::CADDone.value()) == IrqMask::CADDone.value() {
                    debug!("CADDone in radio mode {}", radio_mode);
                    // TODO: don't like how we mutate the cad_activity_detected parameter
                    if cad_activity_detected.is_some() {
                        // Check if the CAD (Channel Activity Detection) Activity Detected flag is set in irq_flags and then update the reference
                        *(cad_activity_detected.unwrap()) =
                            (irq_flags & IrqMask::CADActivityDetected.value()) == IrqMask::CADActivityDetected.value();
                    }
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Sleep | RadioMode::Standby => {
                defmt::warn!("IRQ during sleep/standby?");
            }
            RadioMode::FrequencySynthesis => todo!(),
            RadioMode::Receive(RxMode::DutyCycle(_)) => todo!(),
        }

        // If no specific IRQ condition is met, return None
        Ok(None)
    }
    /// Set the LoRa chip into the TxContinuousWave mode
    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), RadioError> {
        C::set_tx_continuous_wave_mode(self).await
    }
}
