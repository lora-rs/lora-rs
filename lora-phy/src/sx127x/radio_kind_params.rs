use crate::mod_params::*;
use crate::mod_traits::InterfaceVariant;
use crate::sx127x::{
    Sx127x, FREQUENCY_SYNTHESIZER_STEP, SX1272_RSSI_OFFSET, SX1276_RF_MID_BAND_THRESH, SX1276_RSSI_OFFSET_HF,
    SX1276_RSSI_OFFSET_LF,
};
use embedded_hal_async::spi::SpiDevice;

/// Internal sx127x LoRa modes (signified by most significant bit flag)
#[derive(Clone, Copy)]
pub enum LoRaMode {
    Sleep = 0x00,
    Standby = 0x01,
    Tx = 0x03,
    RxContinuous = 0x05,
    RxSingle = 0x06,
    Cad = 0x07,
}

impl LoRaMode {
    /// Mode value, including LoRa flag
    pub fn value(self) -> u8 {
        (self as u8) | 0x80u8
    }
}

// IRQ mapping for sx127x chips:
// DIO0 - RxDone, TxDone, CadDone
// DIO1 - RxTimeout, FhssChangeChannel, CadDetected
// DIO2 - 3x FhssChangeChannel
// DIO3 - CadDone, ValidHeader, PayloadCrcError
// DIO4 - CadDetected, *PllLock, *PllLock
// DIO5 - *ModeReady, *ClkOut, *ClkOut

#[allow(dead_code)]
pub enum DioMapping1Dio0 {
    RxDone = 0x00,
    TxDone = 0x40,
    CadDone = 0x80,
    Other = 0xc0,
    Mask = 0x3f,
}

impl DioMapping1Dio0 {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[allow(dead_code)]
pub enum DioMapping1Dio1 {
    RxTimeOut = 0b00 << 2,
    FhssChangeChannel = 0b01 << 2,
    CadDetected = 0b10 << 2,
    Other = 0b11 << 2,
    Mask = 0xf3,
}

#[allow(dead_code)]
impl DioMapping1Dio1 {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[allow(dead_code)]
pub enum DioMapping1Dio3 {
    CadDone = 0,
    ValidHeader = 0b01,
    PayloadCrcError = 0b10,
    Other = 0b11,
    Mask = 0xfc,
}

impl DioMapping1Dio3 {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum IrqMask {
    None = 0x00,
    CADActivityDetected = 0x01,
    FhssChangedChannel = 0x02,
    CADDone = 0x04,
    TxDone = 0x08,
    HeaderValid = 0x10,
    CRCError = 0x20,
    RxDone = 0x40,
    RxTimeout = 0x80,
    All = 0xFF,
}

impl IrqMask {
    pub fn value(self) -> u8 {
        self as u8
    }

    pub fn is_set_in(self, mask: u8) -> bool {
        self.value() & mask == self.value()
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum Register {
    RegFifo = 0x00,
    RegOpMode = 0x01,
    RegFrfMsb = 0x06,
    RegFrfMid = 0x07,
    RegFrfLsb = 0x08,
    RegPaConfig = 0x09,
    RegPaRamp = 0x0a,
    RegOcp = 0x0b,
    RegLna = 0x0c,
    RegFifoAddrPtr = 0x0d,
    RegFifoTxBaseAddr = 0x0e,
    RegFifoRxBaseAddr = 0x0f,
    RegFifoRxCurrentAddr = 0x10,
    RegIrqFlagsMask = 0x11,
    RegIrqFlags = 0x12,
    RegRxNbBytes = 0x13,
    RegPktSnrValue = 0x19,
    RegModemStat = 0x18,
    RegPktRssiValue = 0x1a,
    RegModemConfig1 = 0x1d,
    RegModemConfig2 = 0x1e,
    RegSymbTimeoutLsb = 0x1f,
    RegPreambleMsb = 0x20,
    RegPreambleLsb = 0x21,
    RegPayloadLength = 0x22,
    RegMaxPayloadLength = 0x23,
    RegModemConfig3 = 0x26,
    RegFreqErrorMsb = 0x28,
    RegFreqErrorMid = 0x29,
    RegFreqErrorLsb = 0x2a,
    RegRssiWideband = 0x2c,
    RegDetectionOptimize = 0x31,
    RegInvertiq = 0x33,
    RegDetectionThreshold = 0x37,
    RegSyncWord = 0x39,
    RegInvertiq2 = 0x3b,
    RegDioMapping1 = 0x40,
    RegVersion = 0x42,
    RegPaDacSX1272 = 0x5a,
    RegPaDacSX1276 = 0x4d,
    RegTcxoSX1276 = 0x4b,
    RegTcxoSX1272 = 0x58,
}

impl Register {
    pub fn read_addr(self) -> u8 {
        (self as u8) & 0x7f
    }
    pub fn write_addr(self) -> u8 {
        (self as u8) | 0x80
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum RampTime {
    Ramp3_4Ms = 0x00,
    Ramp2Ms = 0x01,
    Ramp1Ms = 0x02,
    Ramp500Us = 0x03,
    Ramp250Us = 0x04,
    Ramp125Us = 0x05,
    Ramp100Us = 0x06,
    Ramp62Us = 0x07,
    Ramp50Us = 0x08,
    Ramp40Us = 0x09,
    Ramp31Us = 0x0a,
    Ramp25Us = 0x0b,
    Ramp20Us = 0x0c,
    Ramp15Us = 0x0d,
    Ramp12Us = 0x0e,
    Ramp10Us = 0x0f,
}

impl RampTime {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum LnaGain {
    G1 = 0x20, // maximum gain (default)
    G2 = 0x40,
    G3 = 0x60,
    G4 = 0x80,
    G5 = 0xa0,
    G6 = 0xc0, // minumum gain
}

impl LnaGain {
    pub fn value(self) -> u8 {
        self as u8
    }
    pub fn boosted_value(self) -> u8 {
        (self as u8) | 0x03u8
    }
}

/// PA DAC configuration - sx1276+
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum PaDac {
    _20DbmOn = 0x87,
    _20DbmOff = 0x84,
}

impl PaDac {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// PA configuration - sx1276+
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum PaConfig {
    PaBoost = 0x80,
    MaxPower7NoPaBoost = 0x70,
}

impl PaConfig {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
#[allow(clippy::enum_variant_names)]
pub enum OcpTrim {
    _45Ma = 0x00,
    _50Ma = 0x01,
    _55Ma = 0x02,
    _60Ma = 0x03,
    _65Ma = 0x04,
    _70Ma = 0x05,
    _75Ma = 0x06,
    _80Ma = 0x07,
    _85Ma = 0x08,
    _90Ma = 0x09,
    _95Ma = 0x0a,
    _100Ma = 0x0b,
    _105Ma = 0x0c,
    _110Ma = 0x0d,
    _115Ma = 0x0e,
    _120Ma = 0x0f,
    _130Ma = 0x10,
    _140Ma = 0x11,
    _150Ma = 0x12,
    _160Ma = 0x13,
    _170Ma = 0x14,
    _180Ma = 0x15,
    _190Ma = 0x16,
    _200Ma = 0x17,
    _210Ma = 0x18,
    _220Ma = 0x19,
    _230Ma = 0x1a,
    _240Ma = 0x1b,
}

impl OcpTrim {
    pub fn value(self) -> u8 {
        (self as u8) | 0x20u8 // value plus OCP on flag
    }
}

pub fn spreading_factor_value(spreading_factor: SpreadingFactor) -> Result<u8, RadioError> {
    match spreading_factor {
        SpreadingFactor::_5 => Err(RadioError::UnavailableSpreadingFactor),
        SpreadingFactor::_6 => Ok(0x06),
        SpreadingFactor::_7 => Ok(0x07),
        SpreadingFactor::_8 => Ok(0x08),
        SpreadingFactor::_9 => Ok(0x09),
        SpreadingFactor::_10 => Ok(0x0A),
        SpreadingFactor::_11 => Ok(0x0B),
        SpreadingFactor::_12 => Ok(0x0C),
    }
}

pub trait Sx127xVariant {
    fn bandwidth_value(bw: Bandwidth) -> Result<u8, RadioError>;
    fn reg_txco() -> Register;

    async fn set_tx_power<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        p_out: i32,
        tx_boost: bool,
    ) -> Result<(), RadioError>
    where
        Self: Sized;

    fn ramp_value(ramp_time: RampTime) -> u8;

    async fn set_modulation_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        mdltn_params: &ModulationParams,
    ) -> Result<(), RadioError>
    where
        Self: Sized;
    async fn set_packet_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        pkt_params: &PacketParams,
    ) -> Result<(), RadioError>
    where
        Self: Sized;

    async fn rssi_offset<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<i16, RadioError>
    where
        Self: Sized;
    async fn set_tx_continuous_wave_mode<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<(), RadioError>
    where
        Self: Sized;
}

/// Sx1272 implements the Sx127xVariant trait
pub struct Sx1272;
impl Sx127xVariant for Sx1272 {
    fn bandwidth_value(bw: Bandwidth) -> Result<u8, RadioError> {
        match bw {
            Bandwidth::_125KHz => Ok(0x00),
            Bandwidth::_250KHz => Ok(0x01),
            Bandwidth::_500KHz => Ok(0x02),
            _ => Err(RadioError::UnavailableBandwidth),
        }
    }

    fn reg_txco() -> Register {
        Register::RegTcxoSX1272
    }

    async fn set_tx_power<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        p_out: i32,
        tx_boost: bool,
    ) -> Result<(), RadioError> {
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
                radio.write_register(Register::RegPaConfig, (1 << 7) | val).await?;
                radio.write_register(Register::RegPaDacSX1272, 0x87).await?;
            } else {
                // PA_BOOST out: +2 .. +17 dBm
                let val = (p_out.min(17).max(2) - 2) as u8 & 0x0f;
                radio.write_register(Register::RegPaConfig, (1 << 7) | val).await?;
                radio.write_register(Register::RegPaDacSX1272, 0x84).await?;
            }
        } else {
            // RFO out: -1 to +14 dBm
            let val = (p_out.min(14).max(-1) + 1) as u8 & 0x0f;
            radio.write_register(Register::RegPaConfig, val).await?;
            radio.write_register(Register::RegPaDacSX1272, 0x84).await?;
        }

        Ok(())
    }

    fn ramp_value(ramp_time: RampTime) -> u8 {
        // Sx1272 - default: 0x19
        // [4]: LowPnTxPllOff - use higher power, lower phase noise PLL
        //      only when the transmitter is used (default: 1)
        //      0 - Standard PLL used in Rx mode, Lower PN PLL in Tx
        //      1 - Standard PLL used in both Tx and Rx modes
        ramp_time.value() | (1 << 4)
    }

    async fn set_modulation_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        mdltn_params: &ModulationParams,
    ) -> Result<(), RadioError> {
        let bw_val = Self::bandwidth_value(mdltn_params.bandwidth)?;
        let sf_val = spreading_factor_value(mdltn_params.spreading_factor)?;

        let cfg1 = radio.read_register(Register::RegModemConfig1).await?;
        let ldro = mdltn_params.low_data_rate_optimize;
        let cr_val = coding_rate_value(mdltn_params.coding_rate)?;
        let val = (cfg1 & 0b110) | (bw_val << 6) | (cr_val << 3) | ldro;
        radio.write_register(Register::RegModemConfig1, val).await?;
        let cfg2 = radio.read_register(Register::RegModemConfig2).await?;
        let val = (cfg2 & 0b1111) | (sf_val << 4);
        radio.write_register(Register::RegModemConfig2, val).await?;
        Ok(())
    }

    async fn set_packet_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        pkt_params: &PacketParams,
    ) -> Result<(), RadioError>
    where
        Self: Sized,
    {
        let modemcfg1 = radio.read_register(Register::RegModemConfig1).await?;

        let hdr = pkt_params.implicit_header as u8;
        let crc = pkt_params.crc_on as u8;

        let cfg1 = (modemcfg1 & 0b1111_1001) | (hdr << 2) | (crc << 1);
        radio.write_register(Register::RegModemConfig1, cfg1).await?;
        Ok(())
    }

    async fn rssi_offset<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        _: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<i16, RadioError> {
        Ok(SX1272_RSSI_OFFSET)
    }

    async fn set_tx_continuous_wave_mode<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        _: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<(), RadioError> {
        todo!()
    }
}

/// Sx1276 implements the Sx127xVariant trait
pub struct Sx1276;

impl Sx127xVariant for Sx1276 {
    fn bandwidth_value(bw: Bandwidth) -> Result<u8, RadioError> {
        match bw {
            Bandwidth::_7KHz => Ok(0x00),
            Bandwidth::_10KHz => Ok(0x01),
            Bandwidth::_15KHz => Ok(0x02),
            Bandwidth::_20KHz => Ok(0x03),
            Bandwidth::_31KHz => Ok(0x04),
            Bandwidth::_41KHz => Ok(0x05),
            Bandwidth::_62KHz => Ok(0x06),
            Bandwidth::_125KHz => Ok(0x07),
            Bandwidth::_250KHz => Ok(0x08),
            Bandwidth::_500KHz => Ok(0x09),
        }
    }

    fn reg_txco() -> Register {
        Register::RegTcxoSX1276
    }

    async fn set_tx_power<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        p_out: i32,
        tx_boost: bool,
    ) -> Result<(), RadioError> {
        let pa_reg = Register::RegPaDacSX1276;
        if tx_boost {
            // Output via PA_BOOST: [2, 20] dBm
            let txp = p_out.min(20).max(2);

            // Pout=17-(15-OutputPower)
            let output_power: i32 = txp - 2;

            if txp > 17 {
                radio.write_register(pa_reg, PaDac::_20DbmOn.value()).await?;
                radio.set_ocp(OcpTrim::_240Ma).await?;
            } else {
                radio.write_register(pa_reg, PaDac::_20DbmOff.value()).await?;
                radio.set_ocp(OcpTrim::_100Ma).await?;
            }
            radio
                .write_register(Register::RegPaConfig, PaConfig::PaBoost.value() | (output_power as u8))
                .await?;
        } else {
            // Clamp output: [-4, 14] dBm
            let txp = p_out.min(14).max(-4);

            // Pmax=10.8+0.6*MaxPower, where MaxPower is set below as 7 and therefore Pmax is 15
            // Pout=Pmax-(15-OutputPower)
            let output_power: i32 = txp;

            radio.write_register(pa_reg, PaDac::_20DbmOff.value()).await?;
            radio.set_ocp(OcpTrim::_100Ma).await?;
            radio
                .write_register(
                    Register::RegPaConfig,
                    PaConfig::MaxPower7NoPaBoost.value() | (output_power as u8),
                )
                .await?;
        }
        Ok(())
    }

    fn ramp_value(ramp_time: RampTime) -> u8 {
        // Sx1276 - default: 0x09
        // [4]: reserved (0x00)
        ramp_time as u8
    }

    async fn set_modulation_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        mdltn_params: &ModulationParams,
    ) -> Result<(), RadioError> {
        let bw_val = Self::bandwidth_value(mdltn_params.bandwidth)?;
        let sf_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let coding_rate_denominator_val = coding_rate_denominator_value(mdltn_params.coding_rate)?;

        let mut config_2 = radio.read_register(Register::RegModemConfig2).await?;
        config_2 = (config_2 & 0x0fu8) | ((sf_val << 4) & 0xf0u8);
        radio.write_register(Register::RegModemConfig2, config_2).await?;

        let mut config_1 = radio.read_register(Register::RegModemConfig1).await?;
        config_1 = (config_1 & 0x0fu8) | (bw_val << 4);
        radio.write_register(Register::RegModemConfig1, config_1).await?;

        let cr = coding_rate_denominator_val - 4;
        config_1 = radio.read_register(Register::RegModemConfig1).await?;
        config_1 = (config_1 & 0xf1u8) | (cr << 1);
        radio.write_register(Register::RegModemConfig1, config_1).await?;

        let mut ldro_agc_auto_flags = 0x00u8; // LDRO and AGC Auto both off
        if mdltn_params.low_data_rate_optimize != 0 {
            ldro_agc_auto_flags = 0x08u8; // LDRO on and AGC Auto off
        }
        let mut config_3 = radio.read_register(Register::RegModemConfig3).await?;
        config_3 = (config_3 & 0xf3u8) | ldro_agc_auto_flags;
        radio.write_register(Register::RegModemConfig3, config_3).await
    }

    async fn set_packet_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        pkt_params: &PacketParams,
    ) -> Result<(), RadioError> {
        let mut config_1 = radio.read_register(Register::RegModemConfig1).await?;

        if pkt_params.implicit_header {
            config_1 |= 0x01u8;
        } else {
            config_1 &= 0xfeu8;
        }
        radio.write_register(Register::RegModemConfig1, config_1).await?;

        let mut config_2 = radio.read_register(Register::RegModemConfig2).await?;
        if pkt_params.crc_on {
            config_2 |= 0x04u8;
        } else {
            config_2 &= 0xfbu8;
        }
        radio.write_register(Register::RegModemConfig2, config_2).await?;
        Ok(())
    }

    async fn rssi_offset<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<i16, RadioError> {
        let frequency_in_hz = {
            let msb = radio.read_register(Register::RegFrfMsb).await? as u32;
            let mid = radio.read_register(Register::RegFrfMid).await? as u32;
            let lsb = radio.read_register(Register::RegFrfLsb).await? as u32;
            let frf = (msb << 16) + (mid << 8) + lsb;
            (frf as f64 * FREQUENCY_SYNTHESIZER_STEP) as u32
        };

        if frequency_in_hz > SX1276_RF_MID_BAND_THRESH {
            Ok(SX1276_RSSI_OFFSET_HF)
        } else {
            Ok(SX1276_RSSI_OFFSET_LF)
        }
    }

    async fn set_tx_continuous_wave_mode<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<(), RadioError> {
        radio.intf.iv.enable_rf_switch_rx().await?;
        let pa_config = radio.read_register(Register::RegPaConfig).await?;
        let new_pa_config = pa_config | 0b1000_0000;
        radio.write_register(Register::RegPaConfig, new_pa_config).await?;
        radio.write_register(Register::RegOpMode, 0b1100_0011).await?;
        let modem_config = radio.read_register(Register::RegModemConfig2).await?;
        let new_modem_config = modem_config | 0b0000_1000;
        radio
            .write_register(Register::RegModemConfig2, new_modem_config)
            .await?;
        Ok(())
    }
}

#[allow(dead_code)]
pub fn coding_rate_value(coding_rate: CodingRate) -> Result<u8, RadioError> {
    match coding_rate {
        CodingRate::_4_5 => Ok(0x01),
        CodingRate::_4_6 => Ok(0x02),
        CodingRate::_4_7 => Ok(0x03),
        CodingRate::_4_8 => Ok(0x04),
    }
}

pub fn coding_rate_denominator_value(coding_rate: CodingRate) -> Result<u8, RadioError> {
    match coding_rate {
        CodingRate::_4_5 => Ok(0x05),
        CodingRate::_4_6 => Ok(0x06),
        CodingRate::_4_7 => Ok(0x07),
        CodingRate::_4_8 => Ok(0x08),
    }
}
