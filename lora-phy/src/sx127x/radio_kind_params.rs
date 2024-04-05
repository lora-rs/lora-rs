use crate::mod_params::*;
use crate::mod_traits::InterfaceVariant;
use crate::sx127x::Sx127x;
use embedded_hal_async::spi::SpiDevice;

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
