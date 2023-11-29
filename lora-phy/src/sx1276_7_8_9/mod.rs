mod radio_kind_params;

use defmt::debug;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::*;
use radio_kind_params::*;

use crate::mod_params::*;
use crate::mod_traits::TargetIrqState;
use crate::{InterfaceVariant, RadioKind, SpiInterface};

// Syncwords for public and private networks
const LORA_MAC_PUBLIC_SYNCWORD: u8 = 0x34; // corresponds to sx126x 0x3444
const LORA_MAC_PRIVATE_SYNCWORD: u8 = 0x12; // corresponds to sx126x 0x1424

// TCXO flag
const TCXO_FOR_OSCILLATOR: u8 = 0x10u8;

// Frequency synthesizer step for frequency calculation (Hz)
const FREQUENCY_SYNTHESIZER_STEP: f64 = 61.03515625; // FXOSC (32 MHz) * 1000000 (Hz/MHz) / 524288 (2^19)

/// Supported SX127x chip variants
#[derive(Clone, Copy)]
pub enum Sx127xVariant {
    /// Semtech SX1272
    Sx1272,
    /// Semtech SX1276
    // TODO: should we add variants for 77, 78 and 79 as well?)
    Sx1276,
}

/// Configuration for SX127x-based boards
pub struct Config {
    /// LoRa chip used on specific board
    pub chip: Sx127xVariant,
    /// Whether board is using crystal oscillator or external clock
    pub tcxo_used: bool,
}

/// Base for the RadioKind implementation for the LoRa chip kind and board type
pub struct SX1276_7_8_9<SPI, IV> {
    intf: SpiInterface<SPI, IV>,
    config: Config,
}

impl<SPI, IV> SX1276_7_8_9<SPI, IV>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
{
    /// Create an instance of the RadioKind implementation for the LoRa chip kind and board type
    pub fn new(spi: SPI, iv: IV, config: Config) -> Self {
        let intf = SpiInterface::new(spi, iv);
        Self { intf, config }
    }

    // Utility functions
    async fn write_register(
        &mut self,
        register: Register,
        value: u8,
        is_sleep_command: bool,
    ) -> Result<(), RadioError> {
        let write_buffer = [register.write_addr(), value];
        self.intf.write(&write_buffer, is_sleep_command).await
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

    // Set the number of symbols the radio will wait to detect a reception (maximum 1023 symbols)
    async fn set_lora_symbol_num_timeout(&mut self, symbol_num: u16) -> Result<(), RadioError> {
        if symbol_num > 0x03ffu16 {
            return Err(RadioError::InvalidSymbolTimeout);
        }
        let symbol_num_msb = ((symbol_num & 0x0300u16) >> 8) as u8;
        let symbol_num_lsb = (symbol_num & 0x00ffu16) as u8;
        let mut config_2 = self.read_register(Register::RegModemConfig2).await?;
        config_2 = (config_2 & 0xfcu8) | symbol_num_msb;
        self.write_register(Register::RegModemConfig2, config_2, false).await?;
        self.write_register(Register::RegSymbTimeoutLsb, symbol_num_lsb, false)
            .await
    }

    // Set the over current protection (mA) on the radio
    async fn set_ocp(&mut self, ocp_trim: OcpTrim) -> Result<(), RadioError> {
        self.write_register(Register::RegOcp, ocp_trim.value(), false).await
    }

    /// TODO: tx_boost depends on following:
    /// a) board configuration
    /// b) channel selection
    /// c) other?
    async fn set_tx_power_sx1272(&mut self, p_out: i32, tx_boost: bool) -> Result<(), RadioError> {
        // SX1272 has two output pins:
        // 1) RFO: (-1 to +14 dBm)
        // 2) PA_BOOST: (+2 to +17 dBm and +5 to 20 +dBm)

        // RegPaConfig - 0x32
        // [7] - PaSelect (0: RFO, 1: PA_BOOST)
        // [6:4] - Unused: 0
        // [3:0] - Output power in dB steps

        // RegPaDac - 0x5a (SX1272)
        // [7:3] - Reserved (0x10 as default)
        // [2:0] - PaDac: 0x04 default, 0x07 - enable +20 dBm on PA_BOOST

        // TODO: Shall we also touch OCP settings?
        if tx_boost {
            // Deal with two ranges, +17dBm enables extra boost
            if p_out > 17 {
                // PA_BOOST out: +5 .. +20 dBm
                let val = (p_out.min(20).max(5) - 5) as u8 & 0x0f;
                self.write_register(Register::RegPaConfig, (1 << 7) | val, false)
                    .await?;
                self.write_register(Register::RegPaDacSX1272, 0x87, false).await?;
            } else {
                // PA_BOOST out: +2 .. +17 dBm
                let val = (p_out.min(17).max(2) - 2) as u8 & 0x0f;
                self.write_register(Register::RegPaConfig, (1 << 7) | val, false)
                    .await?;
                self.write_register(Register::RegPaDacSX1272, 0x84, false).await?;
            }
        } else {
            // RFO out: -1 to +14 dBm
            let val = (p_out.min(14).max(-1) + 1) as u8 & 0x0f;
            self.write_register(Register::RegPaConfig, val, false).await?;
            self.write_register(Register::RegPaDacSX1272, 0x84, false).await?;
        }

        Ok(())
    }

    async fn set_tx_power_sx1276(&mut self, p_out: i32, tx_boost: bool) -> Result<(), RadioError> {
        let pa_reg = Register::RegPaDacSX1276;
        if tx_boost {
            // Output via PA_BOOST: [2, 20] dBm
            let txp = p_out.min(20).max(2);

            // Pout=17-(15-OutputPower)
            let output_power: i32 = txp - 2;

            if txp > 17 {
                self.write_register(pa_reg, PaDac::_20DbmOn.value(), false).await?;
                self.set_ocp(OcpTrim::_240Ma).await?;
            } else {
                self.write_register(pa_reg, PaDac::_20DbmOff.value(), false).await?;
                self.set_ocp(OcpTrim::_100Ma).await?;
            }
            self.write_register(
                Register::RegPaConfig,
                PaConfig::PaBoost.value() | (output_power as u8),
                false,
            )
            .await?;
        } else {
            // Clamp output: [-4, 14] dBm
            let txp = p_out.min(14).max(-4);

            // Pmax=10.8+0.6*MaxPower, where MaxPower is set below as 7 and therefore Pmax is 15
            // Pout=Pmax-(15-OutputPower)
            let output_power: i32 = txp;

            self.write_register(pa_reg, PaDac::_20DbmOff.value(), false).await?;
            self.set_ocp(OcpTrim::_100Ma).await?;
            self.write_register(
                Register::RegPaConfig,
                PaConfig::MaxPower7NoPaBoost.value() | (output_power as u8),
                false,
            )
            .await?;
        }
        Ok(())
    }
}

impl<SPI, IV> RadioKind for SX1276_7_8_9<SPI, IV>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
{
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
        self.config.chip.bandwidth_value(bandwidth)?;
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

    // Use DIO2 to control an RF Switch
    async fn init_rf_switch(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_standby(&mut self) -> Result<(), RadioError> {
        self.write_register(Register::RegOpMode, LoRaMode::Standby.value(), false)
            .await?;
        self.intf.iv.disable_rf_switch().await
    }

    async fn set_sleep(&mut self, _warm_start_if_possible: bool, _delay: &mut impl DelayNs) -> Result<(), RadioError> {
        self.intf.iv.disable_rf_switch().await?;
        self.write_register(Register::RegOpMode, LoRaMode::Sleep.value(), true)
            .await?;
        Ok(()) // warm start unavailable for sx127x
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
        if !self.config.tcxo_used {
            return Ok(());
        }

        // Configure Tcxo as input
        let reg = match self.config.chip {
            Sx127xVariant::Sx1272 => Register::RegTcxoSX1272,
            Sx127xVariant::Sx1276 => Register::RegTcxoSX1276,
        };
        self.write_register(reg, TCXO_FOR_OSCILLATOR, false).await
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
        debug!("tx power = {}", p_out);

        // Configure tx power and boost
        match self.config.chip {
            Sx127xVariant::Sx1272 => self.set_tx_power_sx1272(p_out, tx_boosted_if_possible).await,
            Sx127xVariant::Sx1276 => self.set_tx_power_sx1276(p_out, tx_boosted_if_possible).await,
        }?;

        let ramp_time = match is_tx_prep {
            true => RampTime::Ramp40Us,   // for instance, prior to TX or CAD
            false => RampTime::Ramp250Us, // for instance, on initialization
        };
        // Handle chip-specific differences for RegPaRamp 0x0a:
        // Sx1272 - default: 0x19
        // [4]: LowPnTxPllOff - use higher power, lower phase noise PLL
        //      only when the transmitter is used (default: 1)
        //      0 - Standard PLL used in Rx mode, Lower PN PLL in Tx
        //      1 - Standard PLL used in both Tx and Rx modes
        // Sx1276 - default: 0x09
        // [4]: reserved (0x00)
        let val = match self.config.chip {
            Sx127xVariant::Sx1272 => Ok(ramp_time.value() | (1 << 4)),
            Sx127xVariant::Sx1276 => Ok(ramp_time.value()),
        }?;
        self.write_register(Register::RegPaRamp, val, false).await
    }

    async fn update_retention_list(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        let sf_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let bw_val = self.config.chip.bandwidth_value(mdltn_params.bandwidth)?;
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
        self.write_register(Register::RegDetectionOptimize, val, false).await?;
        self.write_register(Register::RegDetectionThreshold, thr, false).await?;
        // Spreading Factor, Bandwidth, codingrate, ldro

        match self.config.chip {
            Sx127xVariant::Sx1272 => {
                let cfg1 = self.read_register(Register::RegModemConfig1).await?;
                let ldro = mdltn_params.low_data_rate_optimize;
                let cr_val = coding_rate_value(mdltn_params.coding_rate)?;
                let val = (cfg1 & 0b110) | (bw_val << 6) | (cr_val << 3) | ldro;
                self.write_register(Register::RegModemConfig1, val, false).await?;
                let cfg2 = self.read_register(Register::RegModemConfig2).await?;
                let val = (cfg2 & 0b1111) | (sf_val << 4);
                self.write_register(Register::RegModemConfig2, val, false).await?;
                Ok(())
            }
            Sx127xVariant::Sx1276 => {
                let mut config_2 = self.read_register(Register::RegModemConfig2).await?;
                config_2 = (config_2 & 0x0fu8) | ((sf_val << 4) & 0xf0u8);
                self.write_register(Register::RegModemConfig2, config_2, false).await?;

                let mut config_1 = self.read_register(Register::RegModemConfig1).await?;
                config_1 = (config_1 & 0x0fu8) | (bw_val << 4);
                self.write_register(Register::RegModemConfig1, config_1, false).await?;

                let cr = coding_rate_denominator_val - 4;
                config_1 = self.read_register(Register::RegModemConfig1).await?;
                config_1 = (config_1 & 0xf1u8) | (cr << 1);
                self.write_register(Register::RegModemConfig1, config_1, false).await?;

                let mut ldro_agc_auto_flags = 0x00u8; // LDRO and AGC Auto both off
                if mdltn_params.low_data_rate_optimize != 0 {
                    ldro_agc_auto_flags = 0x08u8; // LDRO on and AGC Auto off
                }
                let mut config_3 = self.read_register(Register::RegModemConfig3).await?;
                config_3 = (config_3 & 0xf3u8) | ldro_agc_auto_flags;
                self.write_register(Register::RegModemConfig3, config_3, false).await
            }
        }?;

        Ok(())
    }

    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError> {
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

        // TODO: Payload length? (Set when pkt_params.implicit_header == true)?

        let modemcfg1 = self.read_register(Register::RegModemConfig1).await?;

        match self.config.chip {
            Sx127xVariant::Sx1272 => {
                let hdr = (pkt_params.implicit_header == true) as u8;
                let crc = (pkt_params.crc_on == true) as u8;

                let cfg1 = (modemcfg1 & 0b1111_1001) | (hdr << 2) | (crc << 1);
                self.write_register(Register::RegModemConfig1, cfg1, false).await?;
                Ok(())
            }
            Sx127xVariant::Sx1276 => {
                let mut config_1 = modemcfg1;
                if pkt_params.implicit_header {
                    config_1 |= 0x01u8;
                } else {
                    config_1 &= 0xfeu8;
                }
                self.write_register(Register::RegModemConfig1, config_1, false).await?;

                let mut config_2 = self.read_register(Register::RegModemConfig2).await?;
                if pkt_params.crc_on {
                    config_2 |= 0x04u8;
                } else {
                    config_2 &= 0xfbu8;
                }
                self.write_register(Register::RegModemConfig2, config_2, false).await?;
                Ok(())
            }
        }?;

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
        self.write_register(Register::RegInvertiq, (0x13 << 1) | iq1, false)
            .await?;
        self.write_register(Register::RegInvertiq2, iq2, false).await?;
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
    ) -> Result<(), RadioError> {
        if let Some(&_duty_cycle) = duty_cycle_params {
            return Err(RadioError::DutyCycleUnsupported);
        }

        self.intf.iv.enable_rf_switch_rx().await?;

        let mut symbol_timeout_final = symbol_timeout;
        if rx_continuous {
            symbol_timeout_final = 0;
        }
        self.set_lora_symbol_num_timeout(symbol_timeout_final).await?;

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
        self.read_buffer(Register::RegFifo, receiving_buffer).await?;
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

    // Set the IRQ mask to disable unwanted interrupts,
    // enable interrupts on DIO pins (sx127x has multiple),
    // and allow interrupts.
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
                        ^ (IrqMask::RxDone.value()
                            | IrqMask::RxTimeout.value()
                            | IrqMask::CRCError.value()
                            | IrqMask::HeaderValid.value()),
                    false,
                )
                .await?;

                // HeaderValid and CRCError are mutually exclusive when attempting to
                // trigger DIO-based interrupt, so our approach is to trigger HeaderValid
                // as this is required for preamble detection.
                // TODO: RxTimeout should be configured on DIO1
                let dio_mapping_1 = self.read_register(Register::RegDioMapping1).await?;
                let val = (dio_mapping_1 & DioMapping1Dio0::Mask.value() & DioMapping1Dio3::Mask.value())
                    | (DioMapping1Dio0::RxDone.value() | DioMapping1Dio3::ValidHeader.value());
                self.write_register(Register::RegDioMapping1, val, false).await?;

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

    /// Process the radio IRQ.  Log unexpected interrupts, but only bail out on timeout.  Packets from other devices can cause unexpected interrupts.
    async fn process_irq(
        &mut self,
        radio_mode: RadioMode,
        _rx_continuous: bool,
        target_rx_state: TargetIrqState,
        delay: &mut impl DelayNs,
        polling_timeout_in_ms: Option<u32>,
        cad_activity_detected: Option<&mut bool>,
    ) -> Result<TargetIrqState, RadioError> {
        let mut iteration_guard: u32 = 0;
        if polling_timeout_in_ms.is_some() {
            iteration_guard = polling_timeout_in_ms.unwrap();
            iteration_guard /= 50; // poll for interrupts every 50 ms until polling timeout
        }
        let mut i: u32 = 0;
        loop {
            if polling_timeout_in_ms.is_some() && (i >= iteration_guard) {
                return Err(RadioError::PollingTimeout);
            }

            debug!("process_irq loop entered");

            // Await IRQ events unless event polling is used.
            if polling_timeout_in_ms.is_some() {
                delay.delay_ms(50).await;
                i += 1;
            } else {
                self.intf.iv.await_irq().await?;
            }

            let irq_flags = self.read_register(Register::RegIrqFlags).await?;
            self.write_register(Register::RegIrqFlags, 0xffu8, false).await?; // clear all interrupts

            debug!(
                "process_irq: irq_flags = 0b{:08b} in radio mode {}",
                irq_flags, radio_mode
            );

            match radio_mode {
                RadioMode::Transmit => {
                    if (irq_flags & IrqMask::TxDone.value()) == IrqMask::TxDone.value() {
                        debug!("TxDone in radio mode {}", radio_mode);
                        return Ok(TargetIrqState::Done);
                    }
                }
                RadioMode::Receive => {
                    if target_rx_state == TargetIrqState::PreambleReceived && IrqMask::HeaderValid.is_set_in(irq_flags)
                    {
                        debug!("HeaderValid in radio mode {}", radio_mode);
                        return Ok(TargetIrqState::PreambleReceived);
                    }
                    if (irq_flags & IrqMask::RxDone.value()) == IrqMask::RxDone.value() {
                        debug!("RxDone in radio mode {}", radio_mode);
                        return Ok(TargetIrqState::Done);
                    }
                    if (irq_flags & IrqMask::RxTimeout.value()) == IrqMask::RxTimeout.value() {
                        debug!("RxTimeout in radio mode {}", radio_mode);
                        return Err(RadioError::ReceiveTimeout);
                    }
                }
                RadioMode::ChannelActivityDetection => {
                    if (irq_flags & IrqMask::CADDone.value()) == IrqMask::CADDone.value() {
                        debug!("CADDone in radio mode {}", radio_mode);
                        if cad_activity_detected.is_some() {
                            *(cad_activity_detected.unwrap()) = (irq_flags & IrqMask::CADActivityDetected.value())
                                == IrqMask::CADActivityDetected.value();
                        }
                        return Ok(TargetIrqState::Done);
                    }
                }
                RadioMode::Sleep | RadioMode::Standby => {
                    defmt::warn!("IRQ during sleep/standby?");
                }
                RadioMode::FrequencySynthesis | RadioMode::ReceiveDutyCycle => todo!(),
            }

            // if an interrupt occurred for other than an error or operation completion, loop to wait again
        }
    }
    /// Set the LoRa chip into the TxContinuousWave mode
    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), RadioError> {
        match self.config.chip {
            Sx127xVariant::Sx1272 => todo!(),
            Sx127xVariant::Sx1276 => {
                self.intf.iv.enable_rf_switch_rx().await?;
                let pa_config = self.read_register(Register::RegPaConfig).await?;
                let new_pa_config = pa_config | 0b1000_0000;
                self.write_register(Register::RegPaConfig, new_pa_config, false).await?;
                self.write_register(Register::RegOpMode, 0b1100_0011, false).await?;
                let modem_config = self.read_register(Register::RegModemConfig2).await?;
                let new_modem_config = modem_config | 0b0000_1000;
                self.write_register(Register::RegModemConfig2, new_modem_config, false)
                    .await?;
            }
        }
        Ok(())
    }
}
