mod radio_kind_params;
mod sx1280;
pub use sx1280::Sx1280;
mod sx1281;
pub use sx1281::Sx1281;
#[cfg(test)]
mod test;

use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::*;
use radio_kind_params::*;

use crate::mod_params::*;
use crate::mod_traits::IrqState;
use crate::{InterfaceVariant, RadioKind, SpiInterface};

// SWL2001 sx128x_regs.h: "the blob that must be written after wakeup to fix
// the rssi is 0 after a sleep bug"
const RSSI_SNR_BUGFIX_BLOB: [u8; 4] = [0x00, 0xAD, 0x08, 0x9A];

// Frequency synthesizer step: FXOSC (52 MHz) / 2^18 = 198.364 Hz. Same
// fixed-point rounding as SWL2001: the step is scaled by 2^10 so the
// integer division keeps 10 fractional bits, then rounded to nearest.
const PLL_STEP_SHIFT: u32 = 10;
const PLL_STEP_SCALED: u32 = 52_000_000 >> (18 - PLL_STEP_SHIFT);

fn freq_to_pll_step(freq_in_hz: u32) -> u32 {
    let steps_int = freq_in_hz / PLL_STEP_SCALED;
    let steps_frac = freq_in_hz - steps_int * PLL_STEP_SCALED;
    (steps_int << PLL_STEP_SHIFT) + ((steps_frac << PLL_STEP_SHIFT) + (PLL_STEP_SCALED >> 1)) / PLL_STEP_SCALED
}

/// Configuration for SX128x-based boards
pub struct Config<C: Sx128xVariant> {
    /// LoRa chip used on specific board
    pub chip: C,
    /// Whether board is using crystal oscillator or external clock
    pub tcxo_used: bool,
    /// Whether board has the inductor for the DC-DC regulator (Section 14.7.2)
    pub use_dcdc: bool,
}

/// Base for the RadioKind implementation for the LoRa chip kind and board type
pub struct Sx128x<SPI, IV, C: Sx128xVariant + Sized> {
    intf: SpiInterface<SPI, IV>,
    config: Config<C>,
}

impl<SPI, IV, C> Sx128x<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Sx128xVariant,
{
    /// Create an instance of the RadioKind implementation for the LoRa chip kind and board type
    pub fn new(spi: SPI, iv: IV, config: Config<C>) -> Self {
        let intf = SpiInterface::new(spi, iv);
        Self { intf, config }
    }

    // Utility functions
    async fn write_register(&mut self, register: Register, value: u8) -> Result<(), RadioError> {
        let write_buffer = [OpCode::WriteRegister as u8, register.addr1(), register.addr2(), value];
        self.intf.write(&write_buffer, false).await
    }

    async fn read_register(&mut self, register: Register) -> Result<u8, RadioError> {
        let mut read_buffer = [0x00u8; 1];
        self.read_registers(register, &mut read_buffer).await?;
        Ok(read_buffer[0])
    }

    // Consecutive registers starting at `register`
    async fn read_registers(&mut self, register: Register, read_buffer: &mut [u8]) -> Result<(), RadioError> {
        // After the register, a NOP byte is needed.
        let write_buffer = [OpCode::ReadRegister as u8, register.addr1(), register.addr2(), 0x00];
        self.intf.read(&write_buffer, read_buffer).await
    }

    async fn write_registers(&mut self, register: Register, buf: &[u8]) -> Result<(), RadioError> {
        let write_buffer = [OpCode::WriteRegister as u8, register.addr1(), register.addr2()];
        self.intf.write_with_payload(&write_buffer, buf, false).await
    }

    #[cfg(test)]
    fn take_spi(self) -> SPI {
        self.intf.spi
    }

    #[cfg(test)]
    fn spi_mut(&mut self) -> &mut SPI {
        &mut self.intf.spi
    }

    async fn set_packet_type(&mut self, packet_type: PacketType) -> Result<(), RadioError> {
        let write_buffer = [OpCode::SetPacketType as u8, packet_type as u8];
        self.intf.write(&write_buffer, false).await
    }

    /// Preamble length as the chip's exponent/mantissa byte, rounded up to
    /// the next representable length and clamped at the 61440-symbol maximum
    /// (ral_sx128x_convert_lora_pbl_len_from_ral)
    fn preamble_length_value(preamble_length: u16) -> u8 {
        if preamble_length >= 61440 {
            return 0xCF;
        }
        let mut mant = preamble_length;
        let mut exp = 0u8;
        while mant > 1 && mant & 1 == 0 {
            mant >>= 1;
            exp += 1;
        }
        while mant > 15 {
            mant = (mant + 1) >> 1;
            exp += 1;
        }
        (exp << 4) | mant as u8
    }
}

impl<SPI, IV, C> RadioKind for Sx128x<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Sx128xVariant,
{
    // No symbol-count timeout on this chip (SWL2001's RAL reports it
    // unsupported), so every single-shot window is a wall-clock SetRx
    // timeout; a zero ceiling routes them all through RxMode::SingleMs.
    const MAX_SINGLE_RX_SYMBOLS: u16 = 0;
    const SUPPORTS_TIMED_SINGLE_RX: bool = true;

    async fn init_lora(&mut self, sync_word: u16) -> Result<(), RadioError> {
        let mut write_buffer = [0; 16];
        self.read_registers(Register::VersionString, &mut write_buffer).await?;
        debug!("firmware versions: {:?}", write_buffer);
        if b"SX1280" != &write_buffer[0..6] {
            // TODO: maybe introduce explicit error code
            return Err(RadioError::InvalidConfiguration);
        }
        let regulator = if self.config.use_dcdc {
            RegulatorMode::DcDc
        } else {
            RegulatorMode::Ldo
        };
        self.intf
            .write(&[OpCode::SetRegulatorMode as u8, regulator as u8], false)
            .await?;
        self.set_packet_type(PacketType::LoRa).await?;
        self.set_lora_sync_word(sync_word).await?;
        self.set_tx_rx_buffer_base_address(0, 0).await?;
        Ok(())
    }

    async fn set_lora_sync_word(&mut self, sync_word: u16) -> Result<(), RadioError> {
        // The sync word is the upper nibbles of 0x944/0x945; the lower
        // nibbles are chip state and are preserved, as SWL2001 does
        let word = sync_word_to_legacy(sync_word)?;
        let mut regs = [0u8; 2];
        self.read_registers(Register::LoRaSyncWord0, &mut regs).await?;
        regs[0] = (regs[0] & 0x0F) | (word & 0xF0);
        regs[1] = (regs[1] & 0x0F) | (word << 4);
        self.write_registers(Register::LoRaSyncWord0, &regs).await
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
        bandwidth_value(bandwidth)?;
        let low_data_rate_optimize = 0x00u8;
        Ok(ModulationParams {
            spreading_factor,
            bandwidth,
            coding_rate,
            low_data_rate_optimize,
            frequency_in_hz,
        })
    }

    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        let buffer = [
            OpCode::SetModulationParams as u8,
            spreading_factor_value(mdltn_params.spreading_factor)?,
            bandwidth_value(mdltn_params.bandwidth)?,
            coding_rate_value(mdltn_params.coding_rate)?,
        ];
        self.intf.write(&buffer, false).await?;
        // Section 14.4.1: register 0x925 follows the spreading factor
        let sf_cfg = match mdltn_params.spreading_factor {
            SpreadingFactor::_5 | SpreadingFactor::_6 => 0x1E,
            SpreadingFactor::_7 | SpreadingFactor::_8 => 0x37,
            _ => 0x32,
        };
        self.write_register(Register::SfAdditionalConfiguration, sf_cfg).await?;
        // Section 14.4.1: frequency error correction mode 1 in bits 2:0 of 0x93C
        let fec = self.read_register(Register::FrequencyErrorCorrection).await?;
        self.write_register(Register::FrequencyErrorCorrection, (fec & !0x07) | 0x01)
            .await
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

    // The pin reset leaves the chip in STDBY_RC with BUSY low, ready for
    // commands (sx128x_reset in SWL2001 is the HAL reset and nothing else)
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        self.intf.iv.reset(delay).await
    }

    async fn ensure_ready(&mut self, mode: RadioMode) -> Result<(), RadioError> {
        match mode {
            RadioMode::Sleep => {
                // Any NSS assertion wakes the chip; a NOP keeps the bus
                // traffic explicit. BUSY is high until the oscillator runs.
                self.intf.write(&[OpCode::Nop as u8], false).await?;
                // sx128x_wakeup in SWL2001: RSSI reads 0 after a sleep until
                // this undocumented register blob is rewritten. Only a chip
                // that kept its LoRa configuration through the sleep needs
                // it; after a reset the packet type is still GFSK.
                let mut packet_type = [0u8; 1];
                self.intf
                    .read(&[OpCode::GetPacketType as u8, OpCode::Nop as u8], &mut packet_type)
                    .await?;
                if packet_type[0] == PacketType::LoRa as u8 {
                    self.write_registers(Register::RssiSnrBugfix, &RSSI_SNR_BUGFIX_BLOB)
                        .await?;
                }
                Ok(())
            }
            _ => self.intf.iv.wait_on_busy().await,
        }
    }

    async fn set_standby(&mut self) -> Result<(), RadioError> {
        self.intf.iv.disable_rf_switch().await?;
        let buf = [
            OpCode::SetStandby as u8,
            if self.config.tcxo_used {
                StandbyConfig::Xosc
            } else {
                StandbyConfig::Rc
            } as u8,
        ];
        self.intf.write(&buf, false).await
    }

    async fn set_sleep(&mut self, _warm_start_if_possible: bool, _delay: &mut impl DelayNs) -> Result<(), RadioError> {
        // Warm start is unavailable for sx128x
        self.intf.iv.disable_rf_switch().await?;
        // Table 11-17: allow for faster SLEEP to STDBY_RC transition
        let buf = [OpCode::SetSleep as u8, 0x01];
        self.intf.write(&buf, true).await?;
        Ok(())
    }

    async fn set_tx_rx_buffer_base_address(
        &mut self,
        tx_base_addr: usize,
        rx_base_addr: usize,
    ) -> Result<(), RadioError> {
        debug!("sx128x::set_tx_rx_buffer_base_address");
        if tx_base_addr > 255 || rx_base_addr > 255 {
            return Err(RadioError::InvalidBaseAddress(tx_base_addr, rx_base_addr));
        }
        let buf = [
            OpCode::SetBufferBaseAddress as u8,
            tx_base_addr as u8,
            rx_base_addr as u8,
        ];
        self.intf.write(&buf, false).await
    }

    async fn set_tx_power_and_ramp_time(
        &mut self,
        p_out: i32,
        _mdltn_params: Option<&ModulationParams>,
        _is_tx_prep: bool,
    ) -> Result<(), RadioError> {
        // Table 11-49: -18 to +13 dBm, sent with an offset of 18
        if !(-18..=13).contains(&p_out) {
            return Err(RadioError::InvalidOutputPowerForFrequency);
        }
        let power = (p_out + 18) as u8;
        let write_buffer = [OpCode::SetTxParams as u8, power, RampTime::Ramp10Us as u8];
        self.intf.write(&write_buffer, false).await
    }

    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError> {
        // Table 14-50; the last two params are unused by LoRa
        let buffer = [
            OpCode::SetPacketParams as u8,
            Self::preamble_length_value(pkt_params.preamble_length),
            if pkt_params.implicit_header { 0x80 } else { 0x00 },
            pkt_params.payload_length,
            if pkt_params.crc_on { 0x20 } else { 0x00 },
            if pkt_params.iq_inverted { 0x00 } else { 0x40 },
            0,
            0,
        ];
        self.intf.write(&buffer, false).await?;
        // SWL2001 mirrors the IQ polarity into register 0x93B after every
        // SetPacketParams (not in the datasheet)
        let iq = if pkt_params.iq_inverted { 0x0D } else { 0x09 };
        self.write_register(Register::LoRaIqPolarity, iq).await
    }

    // Calibrate the image rejection based on the given frequency
    async fn calibrate_image(&mut self, _frequency_in_hz: u32) -> Result<(), RadioError> {
        // An automatic process, but can set bit ImageCalStart in RegImageCal, when the device is in Standby mode.
        Ok(())
    }

    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        let pll_bytes = freq_to_pll_step(frequency_in_hz).to_be_bytes();
        let buffer = [OpCode::SetRfFrequency as u8, pll_bytes[1], pll_bytes[2], pll_bytes[3]];
        self.intf.write(&buffer, false).await
    }

    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        let write_buffer = [OpCode::WriteBuffer as u8, 0];
        self.intf.write_with_payload(&write_buffer, payload, false).await
    }

    async fn do_tx(&mut self) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;
        // Table 11-22: no timeout; the 1 ms base matches ral_sx128x_set_tx
        let write_buffer = [OpCode::SetTx as u8, PeriodBase::_1ms as u8, 0, 0];
        self.intf.write(&write_buffer, false).await
    }

    async fn do_rx(&mut self, rx_mode: RxMode) -> Result<(), RadioError> {
        self.set_tx_rx_buffer_base_address(0, 0).await?;
        self.clear_irq_status().await?;
        // Table 11-23, same choices as ral_sx128x_set_rx; 0xFFFF on the
        // 1 ms base is continuous, longer windows move to the 4 ms base
        let (period_base, period_base_count) = match rx_mode {
            RxMode::Continuous => (PeriodBase::_1ms, 0xFFFF),
            RxMode::SingleMs(ms) if ms < 0xFFFF => (PeriodBase::_1ms, ms as u16),
            RxMode::SingleMs(ms) => (PeriodBase::_4ms, (ms >> 2).min(0xFFFE) as u16),
            RxMode::Single(_) | RxMode::DutyCycle(_) => return Err(RadioError::InvalidRadioMode),
        };
        let pbb = period_base_count.to_be_bytes();
        let buffer = [OpCode::SetRx as u8, period_base as u8, pbb[0], pbb[1]];
        self.intf.write(&buffer, false).await
    }

    async fn get_rx_payload(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<u8, RadioError> {
        // Table 11-63: payload length, then the offset of the packet in the buffer
        let mut status = [0u8; 2];
        self.intf
            .read(&[OpCode::GetRxBufferStatus as u8, OpCode::Nop as u8], &mut status)
            .await?;
        let payload_length = if rx_pkt_params.implicit_header {
            rx_pkt_params.payload_length
        } else {
            status[0]
        };
        let offset = status[1];
        if (payload_length as usize) > receiving_buffer.len() {
            return Err(RadioError::PayloadSizeMismatch(
                payload_length as usize,
                receiving_buffer.len(),
            ));
        }
        let write_buffer = [OpCode::ReadBuffer as u8, offset, OpCode::Nop as u8];
        self.intf
            .read(&write_buffer, &mut receiving_buffer[0..payload_length as usize])
            .await?;
        Ok(payload_length)
    }

    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError> {
        let write_buffer = [OpCode::GetPacketStatus as u8, 0];
        let mut read_buffer = [0; 5];
        self.intf.read(&write_buffer, &mut read_buffer).await?;
        // Table 11-66: RssiPkt = -raw / 2 dBm, SnrPkt = raw (signed) / 4 dB
        let rssi = Rssi::from_quarter_dbm(-2 * read_buffer[0] as i16);
        let snr = Snr::from_quarter_db(read_buffer[1] as i8 as i16);
        Ok(PacketStatus { rssi, snr })
    }

    async fn get_rssi(&mut self) -> Result<i16, RadioError> {
        let write_buffer = [OpCode::GetRssiInst as u8, 0];
        let mut read_buffer = [0; 1];
        self.intf.read(&write_buffer, &mut read_buffer).await?;
        // Table 11-64: RssiInst = -raw / 2 dBm
        Ok(((-(read_buffer[0] as i32)) >> 1) as i16)
    }

    async fn do_cad(&mut self, _mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;
        // Table 11-56: more symbols means fewer false detections at the cost
        // of a longer scan; 8 is the usual middle ground
        self.intf
            .write(&[OpCode::SetCadParams as u8, CadSymbolNum::_8 as u8], false)
            .await?;
        self.intf.write(&[OpCode::SetCad as u8], false).await
    }

    // Enable the interrupts the mode needs and route them to DIO1
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError> {
        self.clear_irq_status().await?;
        let (irq_mask, dio1_mask) = match radio_mode {
            Some(RadioMode::Transmit) => (
                IrqMask::TxDone.value()
                    | IrqMask::RxDone.value()
                    | IrqMask::SyncWordValid.value()
                    | IrqMask::SyncWordError.value()
                    | IrqMask::CrcError.value(),
                IrqMask::TxDone.value() | IrqMask::RxDone.value(),
            ),
            Some(RadioMode::Receive(_)) => (
                IrqMask::RxDone.value()
                    | IrqMask::HeaderValid.value()
                    | IrqMask::HeaderError.value()
                    | IrqMask::CrcError.value()
                    | IrqMask::RxTxTimeout.value(),
                IrqMask::RxDone.value() | IrqMask::RxTxTimeout.value(),
            ),
            Some(RadioMode::ChannelActivityDetection) => {
                let mask = IrqMask::CadDone.value() | IrqMask::CadDetected.value();
                (mask, mask)
            }
            _ => (IrqMask::None.value(), IrqMask::None.value()),
        };
        let buffer = irq_dio1_buffer_from_params(irq_mask, dio1_mask, 0, 0);
        self.intf.write(&buffer, false).await
    }

    async fn await_irq(&mut self) -> Result<(), RadioError> {
        self.intf.iv.await_irq().await
    }

    async fn get_irq_state(
        &mut self,
        radio_mode: RadioMode,
        cad_activity_detected: Option<&mut bool>,
    ) -> Result<Option<IrqState>, RadioError> {
        // Needs a NOP, see 11.9.2
        let write_buffer = [OpCode::GetIrqStatus as u8, 0];
        let mut read_buffer = [0; 2];
        self.intf.read(&write_buffer, &mut read_buffer).await?;
        let irq_flags = u16::from_be_bytes(read_buffer);
        match radio_mode {
            RadioMode::Receive(RxMode::Continuous | RxMode::Single(_) | RxMode::SingleMs(_)) => {
                if IrqMask::RxDone.is_set_in(irq_flags) {
                    debug!("RxDone in radio mode {}", radio_mode);
                    return Ok(Some(IrqState::Done));
                }
                if IrqMask::RxTxTimeout.is_set_in(irq_flags) {
                    debug!("RxTxTimeout in radio mode {}", radio_mode);
                    return Err(RadioError::ReceiveTimeout);
                }
                if IrqMask::HeaderValid.is_set_in(irq_flags) {
                    debug!("HeaderValid in radio mode {}", radio_mode);
                    return Ok(Some(IrqState::PreambleReceived));
                }
            }
            RadioMode::Transmit => {
                if IrqMask::TxDone.is_set_in(irq_flags) {
                    debug!("TxDone in radio mode {}", radio_mode);
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::ChannelActivityDetection => {
                if IrqMask::CadDone.is_set_in(irq_flags) {
                    if let Some(detected) = cad_activity_detected {
                        *detected = IrqMask::CadDetected.is_set_in(irq_flags);
                    }
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Sleep | RadioMode::Standby | RadioMode::Listen => {
                warn!("IRQ during sleep/standby/listen?");
            }
            RadioMode::FrequencySynthesis | RadioMode::Receive(RxMode::DutyCycle(_)) => {
                return Err(RadioError::InvalidRadioMode);
            }
        }

        // If no specific IRQ condition is met, return None
        Ok(None)
    }

    async fn clear_irq_status(&mut self) -> Result<(), RadioError> {
        debug!("sx128x::clear_irq_status");
        let buffer = [OpCode::ClrIrqStatus as u8, 0xff, 0xff];
        self.intf.write(&buffer, false).await
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
        let irq_state = self.get_irq_state(radio_mode, cad_activity_detected).await;
        if clear_interrupts {
            self.clear_irq_status().await?;
        }
        irq_state
    }

    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;
        self.intf.write(&[OpCode::SetTxContinuousWave as u8], false).await
    }
}

fn irq_dio1_buffer_from_params(irq_mask: u16, dio1_mask: u16, dio2_mask: u16, dio3_mask: u16) -> [u8; 9] {
    let mut buffer = [OpCode::SetDioIrqParams as u8, 0, 0, 0, 0, 0, 0, 0, 0];
    buffer[1..3].copy_from_slice(&irq_mask.to_be_bytes());
    buffer[3..5].copy_from_slice(&dio1_mask.to_be_bytes());
    buffer[5..7].copy_from_slice(&dio2_mask.to_be_bytes());
    buffer[7..9].copy_from_slice(&dio3_mask.to_be_bytes());
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pll_step_rounds_to_nearest() {
        // FXOSC 52 MHz / 2^18
        const STEP_HZ: f64 = 52_000_000.0 / 262_144.0;
        for freq in (2_400_000_000u32..=2_500_000_000).step_by(1_234_567) {
            assert_eq!(freq_to_pll_step(freq), (freq as f64 / STEP_HZ).round() as u32, "{freq}");
        }
    }
}
