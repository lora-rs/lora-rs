mod radio_kind_params;

use defmt::debug;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::*;
pub use radio_kind_params::TcxoCtrlVoltage;
use radio_kind_params::*;

use crate::mod_params::*;
use crate::mod_traits::IrqState;
use crate::{InterfaceVariant, RadioKind, SpiInterface};
mod variant;
pub use variant::*;

// Syncwords for public and private networks
const LORA_MAC_PUBLIC_SYNCWORD: u16 = 0x3444; // corresponds to sx127x 0x34
const LORA_MAC_PRIVATE_SYNCWORD: u16 = 0x1424; // corresponds to sx127x 0x12

// Maximum number of registers that can be added to the retention list
const MAX_NUMBER_REGS_IN_RETENTION: u8 = 4;

// Internal frequency of the radio
const SX126X_XTAL_FREQ: u32 = 32000000;

// Scaling factor used to perform fixed-point operations
const SX126X_PLL_STEP_SHIFT_AMOUNT: u32 = 14;

// PLL step - scaled with SX126X_PLL_STEP_SHIFT_AMOUNT
const SX126X_PLL_STEP_SCALED: u32 = SX126X_XTAL_FREQ >> (25 - SX126X_PLL_STEP_SHIFT_AMOUNT);

// Maximum value for parameter symbNum
const SX126X_MAX_LORA_SYMB_NUM_TIMEOUT: u8 = 248;

// Time required for the TCXO to wakeup [ms].
const BRD_TCXO_WAKEUP_TIME: u32 = 10;

// SetRx timeout argument for enabling continuous mode
const RX_CONTINUOUS_TIMEOUT: u32 = 0xffffff;

/// Power amplifier selection
#[repr(u8)]
pub enum DeviceSel {
    /// Low power, power amplifier, used by sx1261
    LowPowerPA = 1,
    /// High power, power amplifier, used by sx1262
    HighPowerPA = 0,
}

/// Configuration for SX126x-based boards
pub struct Config<C: Sx126xVariant + Sized> {
    /// LoRa chip variant on this board
    pub chip: C,
    /// Board is using TCXO (once enabled DIO3 cannot be used as IRQ)
    pub tcxo_ctrl: Option<TcxoCtrlVoltage>,
    /// Whether board is using optional DCDC in addition to LDO
    pub use_dcdc: bool,
    /// Whether to boost receive
    pub rx_boost: bool,
}

/// Base for the RadioKind implementation for the LoRa chip kind and board type
pub struct Sx126x<SPI, IV, C: Sx126xVariant + Sized> {
    intf: SpiInterface<SPI, IV>,
    config: Config<C>,
}

impl<SPI, IV, C> Sx126x<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Sx126xVariant,
{
    /// Create an instance of the RadioKind implementation for the LoRa chip kind and board type
    pub fn new(spi: SPI, iv: IV, config: Config<C>) -> Self {
        let intf = SpiInterface::new(spi, iv);
        Self { intf, config }
    }

    // Utility functions
    async fn add_register_to_retention_list(&mut self, register: Register) -> Result<(), RadioError> {
        let mut buffer = [0x00u8; (1 + (2 * MAX_NUMBER_REGS_IN_RETENTION)) as usize];

        // Read the address and registers already added to the list
        self.intf
            .read(
                &[
                    OpCode::ReadRegister.value(),
                    Register::RetentionList.addr1(),
                    Register::RetentionList.addr2(),
                    0x00u8,
                ],
                &mut buffer,
            )
            .await?;

        let number_of_registers = buffer[0];
        for i in 0..number_of_registers {
            if register.addr1() == buffer[(1 + (2 * i)) as usize] && register.addr2() == buffer[(2 + (2 * i)) as usize]
            {
                return Ok(()); // register already in list
            }
        }

        if number_of_registers < MAX_NUMBER_REGS_IN_RETENTION {
            buffer[0] += 1; // increment number of registers

            buffer[(1 + (2 * number_of_registers)) as usize] = register.addr1();
            buffer[(2 + (2 * number_of_registers)) as usize] = register.addr2();

            let register = [
                OpCode::WriteRegister.value(),
                Register::RetentionList.addr1(),
                Register::RetentionList.addr2(),
            ];
            self.intf.write_with_payload(&register, &buffer, false).await
        } else {
            Err(RadioError::InvalidConfiguration)
        }
    }

    async fn update_retention_list(&mut self) -> Result<(), RadioError> {
        self.add_register_to_retention_list(Register::RxGain).await?;
        self.add_register_to_retention_list(Register::TxModulation).await
    }

    // Set the number of symbols the radio will wait to detect a reception
    async fn set_lora_symbol_num_timeout(&mut self, symbol_num: u16) -> Result<(), RadioError> {
        let mut exp = 0u8;
        let mut mant = ((symbol_num.min(SX126X_MAX_LORA_SYMB_NUM_TIMEOUT.into()) + 1) >> 1) as u8;
        while mant > 31 {
            mant = (mant + 3) >> 2;
            exp += 1;
        }
        let val: u8 = mant << ((2 * exp) + 1);
        self.intf
            .write(&[OpCode::SetLoRaSymbTimeout.value(), val], false)
            .await?;

        if symbol_num > 0 {
            let val = exp + (mant << 3);
            let buf = [
                OpCode::WriteRegister.value(),
                Register::SynchTimeout.addr1(),
                Register::SynchTimeout.addr2(),
                val,
            ];
            self.intf.write(&buf, false).await?;
        }
        Ok(())
    }

    async fn set_pa_config(&mut self, pa_duty_cycle: u8, hp_max: u8, device_sel: DeviceSel) -> Result<(), RadioError> {
        const PA_LUT_RESERVED: u8 = 0x01;
        let op_code_and_pa_config = [
            OpCode::SetPAConfig.value(),
            pa_duty_cycle,
            hp_max,
            device_sel as u8,
            PA_LUT_RESERVED,
        ];
        self.intf.write(&op_code_and_pa_config, false).await
    }

    fn timeout_1(timeout: u32) -> u8 {
        ((timeout >> 16) & 0xFF) as u8
    }
    fn timeout_2(timeout: u32) -> u8 {
        ((timeout >> 8) & 0xFF) as u8
    }
    fn timeout_3(timeout: u32) -> u8 {
        (timeout & 0xFF) as u8
    }

    fn convert_freq_in_hz_to_pll_step(freq_in_hz: u32) -> u32 {
        // Get integer and fractional parts of the frequency computed with a PLL step scaled value
        let steps_int = freq_in_hz / SX126X_PLL_STEP_SCALED;
        let steps_frac = freq_in_hz - (steps_int * SX126X_PLL_STEP_SCALED);

        (steps_int << SX126X_PLL_STEP_SHIFT_AMOUNT)
            + (((steps_frac << SX126X_PLL_STEP_SHIFT_AMOUNT) + (SX126X_PLL_STEP_SCALED >> 1)) / SX126X_PLL_STEP_SCALED)
    }
}

impl<SPI, IV, C> RadioKind for Sx126x<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Sx126xVariant,
{
    async fn init_lora(&mut self, is_public_network: bool) -> Result<(), RadioError> {
        // DC-DC regulator setup (default is LDO)
        if self.config.use_dcdc {
            let reg_data = [OpCode::SetRegulatorMode.value(), RegulatorMode::UseDCDC.value()];
            self.intf.write(&reg_data, false).await?;
        }
        // DIO2 acting as RF Switch (default is DIO2 as IRQ)
        if self.config.chip.use_dio2_as_rfswitch() {
            let cmd = [
                OpCode::SetDIO2AsRfSwitchCtrl.value(),
                self.config.chip.use_dio2_as_rfswitch() as u8,
            ];
            self.intf.write(&cmd, false).await?;
        }

        // DIO3 acting as TCXO controller (default is DIO3 as IRQ)
        if let Some(voltage) = self.config.tcxo_ctrl {
            // When TCXO is used, XOSC_START_ERR flag is raised at POR or at
            // wake-up from Sleep mode in cold-start condition. This is an
            // expected behaviour since chip is not yet aware of being clocked
            // by TCXO and therefore this should be initially cleared manually.
            let mut buf = [0u8; 2];
            let _ = self
                .intf
                .read_with_status(&[OpCode::ClearDeviceErrors.value()], &mut buf)
                .await?;

            // Each unit is 15.625uS (which is 1/64th ms)
            let timeout = BRD_TCXO_WAKEUP_TIME << 6;
            let op_code_and_tcxo_control = [
                OpCode::SetTCXOMode.value(),
                voltage.value() & 0x07,
                Self::timeout_1(timeout),
                Self::timeout_2(timeout),
                Self::timeout_3(timeout),
            ];
            self.intf.write(&op_code_and_tcxo_control, false).await?;
            // Re-run calibration now that chip knows that it's running from TCXO
            self.intf
                .write(&[OpCode::Calibrate.value(), 0b0111_1111], false)
                .await?;
            self.intf.iv.wait_on_busy().await?;
        }

        // Enable LoRa packet engine...
        self.intf
            .write(&[OpCode::SetPacketType.value(), PacketType::LoRa.value()], false)
            .await?;
        let word = match is_public_network {
            true => u16::to_be_bytes(LORA_MAC_PUBLIC_SYNCWORD),
            false => u16::to_be_bytes(LORA_MAC_PRIVATE_SYNCWORD),
        };
        // ...and network syncword
        let lora_syncword_set = [
            OpCode::WriteRegister.value(),
            Register::LoRaSyncword.addr1(),
            Register::LoRaSyncword.addr2(),
            word[0],
            word[1],
        ];
        self.intf.write(&lora_syncword_set, false).await?;

        self.set_tx_rx_buffer_base_address(0, 0).await?;
        // Update register list to support warm starts from sleep mode
        self.update_retention_list().await?;
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
        bandwidth_value(bandwidth)?;
        coding_rate_value(coding_rate)?;
        if ((bandwidth == Bandwidth::_250KHz) || (bandwidth == Bandwidth::_500KHz)) && (frequency_in_hz < 400_000_000) {
            return Err(RadioError::InvalidBandwidthForFrequency);
        }

        let mut low_data_rate_optimize = 0x00u8;
        if (((spreading_factor == SpreadingFactor::_11) || (spreading_factor == SpreadingFactor::_12))
            && (bandwidth == Bandwidth::_125KHz))
            || ((spreading_factor == SpreadingFactor::_12) && (bandwidth == Bandwidth::_250KHz))
        {
            low_data_rate_optimize = 0x01u8;
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
        mut preamble_length: u16,
        implicit_header: bool,
        payload_length: u8,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: &ModulationParams,
    ) -> Result<PacketParams, RadioError> {
        if ((modulation_params.spreading_factor == SpreadingFactor::_5)
            || (modulation_params.spreading_factor == SpreadingFactor::_6))
            && (preamble_length < 12)
        {
            preamble_length = 12;
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
        self.intf.iv.reset(delay).await
    }

    // Wakeup the radio if it is in Sleep or ReceiveDutyCycle mode; otherwise, ensure it is not busy.
    async fn ensure_ready(&mut self, mode: RadioMode) -> Result<(), RadioError> {
        match mode {
            RadioMode::Sleep | RadioMode::Receive(RxMode::DutyCycle(_)) => {
                let op_code_and_null = [OpCode::GetStatus.value(), 0x00u8];
                self.intf.write(&op_code_and_null, false).await?;
            }
            _ => self.intf.iv.wait_on_busy().await?,
        }
        Ok(())
    }

    // Use standby mode RC (not XOSC).
    async fn set_standby(&mut self) -> Result<(), RadioError> {
        let op_code_and_standby_mode = [OpCode::SetStandby.value(), StandbyMode::RC.value()];
        self.intf.write(&op_code_and_standby_mode, false).await?;
        self.intf.iv.disable_rf_switch().await
    }

    async fn set_sleep(&mut self, warm_start_if_possible: bool, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        self.intf.iv.disable_rf_switch().await?;
        let sleep_params = SleepParams {
            wakeup_rtc: false,
            reset: false,
            warm_start: warm_start_if_possible,
        };
        let op_code_and_sleep_params = [OpCode::SetSleep.value(), sleep_params.value()];
        self.intf.write(&op_code_and_sleep_params, true).await?;
        delay.delay_ms(2).await;

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
        let op_code_and_base_addrs = [
            OpCode::SetBufferBaseAddress.value(),
            tx_base_addr as u8,
            rx_base_addr as u8,
        ];
        self.intf.write(&op_code_and_base_addrs, false).await
    }

    // Set parameters associated with power for a send operation. Currently, over current protection (OCP) uses the default set automatically after set_pa_config()
    //   output_power            desired RF output power (dBm)
    //   mdltn_params            needed for a power vs channel frequency validation
    //   tx_boosted_if_possible  not pertinent for sx126x
    //   is_tx_prep              indicates which ramp up time to use
    async fn set_tx_power_and_ramp_time(
        &mut self,
        output_power: i32,
        mdltn_params: Option<&ModulationParams>,
        is_tx_prep: bool,
    ) -> Result<(), RadioError> {
        let tx_params_power;
        let ramp_time = match is_tx_prep {
            true => RampTime::Ramp40Us,   // for instance, prior to TX or CAD
            false => RampTime::Ramp200Us, // for instance, on initialization
        };

        match self.config.chip.get_device_sel() {
            DeviceSel::LowPowerPA => {
                const LOW_POWER_MIN: i32 = -17;
                const LOW_POWER_MAX: i32 = 15;
                // Clamp power between [-17, 15] dBm
                let txp = output_power.clamp(LOW_POWER_MIN, LOW_POWER_MAX);

                if txp == 15 {
                    if let Some(m_p) = mdltn_params {
                        if m_p.frequency_in_hz < 400_000_000 {
                            return Err(RadioError::InvalidOutputPowerForFrequency);
                        }
                    }
                }

                // For SX1261:
                // if f < 400 MHz, paDutyCycle should not be higher than 0x04,
                // if f > 400 Mhz, paDutyCycle should not be higher than 0x07.
                // From Table 13-21: PA Operating Modes with Optimal Settings
                match txp {
                    LOW_POWER_MAX => {
                        self.set_pa_config(0x06, 0x00, DeviceSel::LowPowerPA).await?;
                        tx_params_power = 14;
                    }
                    11..=14 => {
                        self.set_pa_config(0x04, 0x00, DeviceSel::LowPowerPA).await?;
                        tx_params_power = txp as u8;
                    }
                    // 10 and less
                    LOW_POWER_MIN..=10 => {
                        self.set_pa_config(0x01, 0x00, DeviceSel::LowPowerPA).await?;
                        // table indicates 10 dBm => txp = 13, therefore we add 3 to values below 10
                        tx_params_power = txp as u8 + 3;
                    }
                    _ => unreachable!("Invalid output power value for low power PA!"),
                }
            }
            DeviceSel::HighPowerPA => {
                const HIGH_POWER_MIN: i32 = -9;
                const HIGH_POWER_MAX: i32 = 22;
                // Clamp power between [-9, 22] dBm
                let txp = output_power.clamp(HIGH_POWER_MIN, HIGH_POWER_MAX);
                // Provide better resistance of the SX1262 Tx to antenna mismatch (see DS_SX1261-2_V1.2 datasheet chapter 15.2)
                let mut tx_clamp_cfg = [0x00u8];
                self.intf
                    .read(
                        &[
                            OpCode::ReadRegister.value(),
                            Register::TxClampCfg.addr1(),
                            Register::TxClampCfg.addr2(),
                            0x00u8,
                        ],
                        &mut tx_clamp_cfg,
                    )
                    .await?;
                tx_clamp_cfg[0] |= 0x0F << 1;
                let register_and_tx_clamp_cfg = [
                    OpCode::WriteRegister.value(),
                    Register::TxClampCfg.addr1(),
                    Register::TxClampCfg.addr2(),
                    tx_clamp_cfg[0],
                ];
                self.intf.write(&register_and_tx_clamp_cfg, false).await?;

                // From Table 13-21: PA Operating Modes with Optimal Settings
                match txp {
                    21..=HIGH_POWER_MAX => {
                        self.set_pa_config(0x04, 0x07, DeviceSel::HighPowerPA).await?;
                        tx_params_power = 22;
                    }
                    18..=20 => {
                        self.set_pa_config(0x03, 0x05, DeviceSel::HighPowerPA).await?;
                        // table indicates 20 dBm => txp = 22, therefore we add 2 to this range
                        tx_params_power = txp as u8 + 2;
                    }
                    15..=17 => {
                        self.set_pa_config(0x02, 0x03, DeviceSel::HighPowerPA).await?;
                        // table indicates 17 dBm => txp = 22, therefore we add 5 to this range
                        tx_params_power = txp as u8 + 5;
                    }
                    HIGH_POWER_MIN..=14 => {
                        self.set_pa_config(0x02, 0x02, DeviceSel::HighPowerPA).await?;
                        // table indicates 14 dBm => txp = 22, therefore we add 8 to this range
                        tx_params_power = txp as u8 + 8;
                    }
                    _ => {
                        unreachable!("Invalid output power value for high power PA!")
                    }
                }
            }
        }
        let op_code_and_tx_params = [OpCode::SetTxParams.value(), tx_params_power, ramp_time.value()];
        self.intf.write(&op_code_and_tx_params, false).await
    }

    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        let spreading_factor_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let bandwidth_val = bandwidth_value(mdltn_params.bandwidth)?;
        let coding_rate_val = coding_rate_value(mdltn_params.coding_rate)?;
        debug!(
            "sf = {}, bw = {}, cr = {}",
            spreading_factor_val, bandwidth_val, coding_rate_val
        );
        let op_code_and_mod_params = [
            OpCode::SetModulationParams.value(),
            spreading_factor_val,
            bandwidth_val,
            coding_rate_val,
            mdltn_params.low_data_rate_optimize,
        ];
        self.intf.write(&op_code_and_mod_params, false).await?;

        // Handle modulation quality with the 500 kHz LoRa bandwidth (see DS_SX1261-2_V1.2 datasheet chapter 15.1)
        let mut tx_mod = [0x00u8];
        self.intf
            .read(
                &[
                    OpCode::ReadRegister.value(),
                    Register::TxModulation.addr1(),
                    Register::TxModulation.addr2(),
                    0x00u8,
                ],
                &mut tx_mod,
            )
            .await?;
        if mdltn_params.bandwidth == Bandwidth::_500KHz {
            let register_and_tx_mod_update = [
                OpCode::WriteRegister.value(),
                Register::TxModulation.addr1(),
                Register::TxModulation.addr2(),
                tx_mod[0] & (!(1 << 2)),
            ];
            self.intf.write(&register_and_tx_mod_update, false).await
        } else {
            let register_and_tx_mod_update = [
                OpCode::WriteRegister.value(),
                Register::TxModulation.addr1(),
                Register::TxModulation.addr2(),
                tx_mod[0] | (1 << 2),
            ];
            self.intf.write(&register_and_tx_mod_update, false).await
        }
    }

    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError> {
        let op_code_and_pkt_params = [
            OpCode::SetPacketParams.value(),
            ((pkt_params.preamble_length >> 8) & 0xFF) as u8,
            (pkt_params.preamble_length & 0xFF) as u8,
            pkt_params.implicit_header as u8,
            pkt_params.payload_length,
            pkt_params.crc_on as u8,
            pkt_params.iq_inverted as u8,
        ];
        self.intf.write(&op_code_and_pkt_params, false).await?;

        // Optimize Inverted IQ Operation, otherwise packet loss with longer packets might occur.
        let mut iq_polarity = [0x00u8];
        self.intf
            .read(
                &[
                    OpCode::ReadRegister.value(),
                    Register::IQPolarity.addr1(),
                    Register::IQPolarity.addr2(),
                    0x00u8,
                ],
                &mut iq_polarity,
            )
            .await?;

        let reg = if pkt_params.iq_inverted {
            iq_polarity[0] & (!(1 << 2))
        } else {
            iq_polarity[0] | (1 << 2)
        };

        let op = [
            OpCode::WriteRegister.value(),
            Register::IQPolarity.addr1(),
            Register::IQPolarity.addr2(),
            reg,
        ];
        self.intf.write(&op, false).await?;
        Ok(())
    }

    // Calibrate the image rejection based on the given frequency
    async fn calibrate_image(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
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
        self.intf.write(&op_code_and_cal_freq, false).await
    }

    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        debug!("channel = {}", frequency_in_hz);
        let freq_in_pll_steps = Self::convert_freq_in_hz_to_pll_step(frequency_in_hz);
        let op_code_and_pll_steps = [
            OpCode::SetRFFrequency.value(),
            ((freq_in_pll_steps >> 24) & 0xFF) as u8,
            ((freq_in_pll_steps >> 16) & 0xFF) as u8,
            ((freq_in_pll_steps >> 8) & 0xFF) as u8,
            (freq_in_pll_steps & 0xFF) as u8,
        ];
        self.intf.write(&op_code_and_pll_steps, false).await
    }

    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        let op_code_and_offset = [OpCode::WriteBuffer.value(), 0x00u8];
        self.intf.write_with_payload(&op_code_and_offset, payload, false).await
    }

    async fn do_tx(&mut self) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;

        // Disable timeout
        let cmd = [
            OpCode::SetTx.value(),
            Self::timeout_1(0),
            Self::timeout_2(0),
            Self::timeout_3(0),
        ];
        self.intf.write(&cmd, false).await
    }

    async fn do_rx(&mut self, rx_mode: RxMode) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;

        // Stop the Rx timer on preamble detection
        let op_code_and_true_flag = [OpCode::SetStopRxTimerOnPreamble.value(), 0x01u8];
        self.intf.write(&op_code_and_true_flag, false).await?;

        let num_symbols = match rx_mode {
            RxMode::DutyCycle(_) | RxMode::Continuous => 0,
            RxMode::Single(n) => n,
        };
        self.set_lora_symbol_num_timeout(num_symbols).await?;

        let rx_gain = if self.config.rx_boost { 0x96 } else { 0x94 };
        let register_and_rx_gain = [
            OpCode::WriteRegister.value(),
            Register::RxGain.addr1(),
            Register::RxGain.addr2(),
            rx_gain,
        ];
        self.intf.write(&register_and_rx_gain, false).await?;

        match rx_mode {
            RxMode::DutyCycle(args) => {
                let op = [
                    OpCode::SetRxDutyCycle.value(),
                    Self::timeout_1(args.rx_time),
                    Self::timeout_2(args.rx_time),
                    Self::timeout_3(args.rx_time),
                    Self::timeout_1(args.sleep_time),
                    Self::timeout_2(args.sleep_time),
                    Self::timeout_3(args.sleep_time),
                ];
                self.intf.write(&op, false).await
            }
            RxMode::Single(_) => {
                let op = [
                    OpCode::SetRx.value(),
                    Self::timeout_1(0),
                    Self::timeout_2(0),
                    Self::timeout_3(0),
                ];
                self.intf.write(&op, false).await
            }
            RxMode::Continuous => {
                let op = [
                    OpCode::SetRx.value(),
                    Self::timeout_1(RX_CONTINUOUS_TIMEOUT),
                    Self::timeout_2(RX_CONTINUOUS_TIMEOUT),
                    Self::timeout_3(RX_CONTINUOUS_TIMEOUT),
                ];
                self.intf.write(&op, false).await
            }
        }
    }

    async fn get_rx_payload(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<u8, RadioError> {
        let op_code = [OpCode::GetRxBufferStatus.value()];
        let mut rx_buffer_status = [0x00u8; 2];
        let read_status = self.intf.read_with_status(&op_code, &mut rx_buffer_status).await?;
        if OpStatusErrorMask::is_error(read_status) {
            return Err(RadioError::OpError(read_status));
        }

        let mut payload_length_buffer = [0x00u8];
        if rx_pkt_params.implicit_header {
            self.intf
                .read(
                    &[
                        OpCode::ReadRegister.value(),
                        Register::PayloadLength.addr1(),
                        Register::PayloadLength.addr2(),
                        0x00u8,
                    ],
                    &mut payload_length_buffer,
                )
                .await?;
        } else {
            payload_length_buffer[0] = rx_buffer_status[0];
        }

        let payload_length = payload_length_buffer[0];
        let offset = rx_buffer_status[1];

        if (payload_length as usize) > receiving_buffer.len() {
            Err(RadioError::PayloadSizeMismatch(
                payload_length as usize,
                receiving_buffer.len(),
            ))
        } else {
            self.intf
                .read(
                    &[OpCode::ReadBuffer.value(), offset, 0x00u8],
                    &mut receiving_buffer[..payload_length as usize],
                )
                .await?;
            Ok(payload_length)
        }
    }

    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError> {
        let op_code = [OpCode::GetPacketStatus.value()];
        let mut pkt_status = [0x00u8; 3];
        let read_status = self.intf.read_with_status(&op_code, &mut pkt_status).await?;
        if OpStatusErrorMask::is_error(read_status) {
            return Err(RadioError::OpError(read_status));
        }
        // check this ???
        let rssi = ((-(pkt_status[0] as i32)) >> 1) as i16;
        let snr = (((pkt_status[1] as i8) + 2) >> 2) as i16;
        let _signal_rssi = ((-(pkt_status[2] as i32)) >> 1) as i16; // unused currently

        Ok(PacketStatus { rssi, snr })
    }

    async fn do_cad(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;

        let mut rx_gain_final = 0x94u8;
        // if Rx boosted, set max LNA gain, increase current by ~2mA for around ~3dB in sensitivity
        if self.config.rx_boost {
            rx_gain_final = 0x96u8;
        }

        let register_and_rx_gain = [
            OpCode::WriteRegister.value(),
            Register::RxGain.addr1(),
            Register::RxGain.addr2(),
            rx_gain_final,
        ];
        self.intf.write(&register_and_rx_gain, false).await?;

        // See:
        //  https://lora-developers.semtech.com/documentation/tech-papers-and-guides/channel-activity-detection-ensuring-your-lora-packets-are-sent/how-to-ensure-your-lora-packets-are-sent-properly
        // for default values used here.
        let spreading_factor_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let op_code_and_cad_params = [
            OpCode::SetCADParams.value(),
            CADSymbols::_8.value(),      // number of symbols for detection
            spreading_factor_val + 13u8, // limit for detection of SNR peak
            10u8,                        // minimum symbol recognition
            0x00u8,                      // CAD exit mode without listen-before-send or subsequent receive processing
            0x00u8,                      // no timeout
            0x00u8,
            0x00u8,
        ];
        self.intf.write(&op_code_and_cad_params, false).await?;

        let op_code_for_set_cad = [OpCode::SetCAD.value()];
        self.intf.write(&op_code_for_set_cad, false).await
    }

    // Set the IRQ mask and DIO masks
    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError> {
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
            Some(RadioMode::Receive(_)) => {
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
            (dio3_mask & 0x00FF) as u8,
        ];
        self.intf.write(&op_code_and_masks, false).await
    }

    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;

        let op_code = [OpCode::SetTxContinuousWave.value()];
        self.intf.write(&op_code, false).await
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
        let op_code = [OpCode::GetIrqStatus.value()];
        let mut irq_status = [0x00u8, 0x00u8];
        // Assuming intf.read_with_status is an existing async method that reads the IRQ status.
        let read_status = self.intf.read_with_status(&op_code, &mut irq_status).await?;
        let irq_flags = ((irq_status[0] as u16) << 8) | (irq_status[1] as u16);

        if clear_interrupts && irq_flags != 0 {
            let op_code_and_irq_status = [OpCode::ClrIrqStatus.value(), irq_status[0], irq_status[1]];
            self.intf.write(&op_code_and_irq_status, false).await?;
        }

        if OpStatusErrorMask::is_error(read_status) {
            debug!(
                "process_irq read status error = 0x{:x} in radio mode {}",
                read_status, radio_mode
            );
        }

        debug!(
            "process_irq satisfied: irq_flags = 0x{:x} in radio mode {}",
            irq_flags, radio_mode
        );

        if (irq_flags & IrqMask::HeaderValid.value()) == IrqMask::HeaderValid.value() {
            debug!("HeaderValid in radio mode {}", radio_mode);
        }
        if (irq_flags & IrqMask::PreambleDetected.value()) == IrqMask::PreambleDetected.value() {
            debug!("PreambleDetected in radio mode {}", radio_mode);
        }
        if (irq_flags & IrqMask::SyncwordValid.value()) == IrqMask::SyncwordValid.value() {
            debug!("SyncwordValid in radio mode {}", radio_mode);
        }

        match radio_mode {
            RadioMode::Transmit => {
                if (irq_flags & IrqMask::TxDone.value()) == IrqMask::TxDone.value() {
                    return Ok(Some(IrqState::Done));
                }
                if (irq_flags & IrqMask::RxTxTimeout.value()) == IrqMask::RxTxTimeout.value() {
                    return Err(RadioError::TransmitTimeout);
                }
            }
            RadioMode::Receive(rx_mode) => {
                if (irq_flags & IrqMask::HeaderError.value()) == IrqMask::HeaderError.value() {
                    debug!("HeaderError in radio mode {}", radio_mode);
                }
                if (irq_flags & IrqMask::CRCError.value()) == IrqMask::CRCError.value() {
                    debug!("CRCError in radio mode {}", radio_mode);
                }
                if (irq_flags & IrqMask::RxDone.value()) == IrqMask::RxDone.value() {
                    debug!("RxDone in radio mode {}", radio_mode);
                    if rx_mode != RxMode::Continuous {
                        // implicit header mode timeout behavior (see DS_SX1261-2_V1.2 datasheet chapter 15.3)
                        let register_and_clear = [
                            OpCode::WriteRegister.value(),
                            Register::RTCCtrl.addr1(),
                            Register::RTCCtrl.addr2(),
                            0x00u8,
                        ];
                        self.intf.write(&register_and_clear, false).await?;

                        let mut evt_clr = [0x00u8];
                        self.intf
                            .read(
                                &[
                                    OpCode::ReadRegister.value(),
                                    Register::EvtClr.addr1(),
                                    Register::EvtClr.addr2(),
                                    0x00u8,
                                ],
                                &mut evt_clr,
                            )
                            .await?;
                        evt_clr[0] |= 1 << 1;
                        let register_and_evt_clear = [
                            OpCode::WriteRegister.value(),
                            Register::EvtClr.addr1(),
                            Register::EvtClr.addr2(),
                            evt_clr[0],
                        ];
                        self.intf.write(&register_and_evt_clear, false).await?;
                    }
                    return Ok(Some(IrqState::Done));
                }
                if IrqMask::PreambleDetected.is_set_in(irq_flags) || IrqMask::HeaderValid.is_set_in(irq_flags) {
                    return Ok(Some(IrqState::PreambleReceived));
                }
                if (irq_flags & IrqMask::RxTxTimeout.value()) == IrqMask::RxTxTimeout.value() {
                    return Err(RadioError::ReceiveTimeout);
                }
            }
            RadioMode::ChannelActivityDetection => {
                if (irq_flags & IrqMask::CADDone.value()) == IrqMask::CADDone.value() {
                    if let Some(detected) = cad_activity_detected {
                        *detected =
                            (irq_flags & IrqMask::CADActivityDetected.value()) == IrqMask::CADActivityDetected.value();
                    }
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Sleep | RadioMode::Standby => {
                defmt::warn!("IRQ during sleep/standby?");
            }
            RadioMode::FrequencySynthesis => todo!(),
        }

        // If none of the specific conditions are met, return None to indicate no IRQ state change.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    // -17 (0xEF) to +14 (0x0E) dBm by step of 1 dB if low power PA is selected
    // -9 (0xF7) to +22 (0x16) dBm by step of 1 dB if high power PA is selected
    fn power_level_negative_value_conversion() {
        let mut i32_val: i32 = -17;
        assert_eq!(i32_val as u8, 0xefu8);
        i32_val = -9;
        assert_eq!(i32_val as u8, 0xf7u8);
    }
}
