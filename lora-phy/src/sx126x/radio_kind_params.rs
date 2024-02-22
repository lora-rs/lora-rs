use crate::mod_params::*;

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub enum PacketType {
    GFSK = 0x00,
    LoRa = 0x01,
    None = 0x0F,
}

impl PacketType {
    pub const fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
pub enum IrqMask {
    None = 0x0000,
    TxDone = 0x0001,
    RxDone = 0x0002,
    PreambleDetected = 0x0004,
    SyncwordValid = 0x0008,
    HeaderValid = 0x0010,
    HeaderError = 0x0020,
    CRCError = 0x0040,
    CADDone = 0x0080,
    CADActivityDetected = 0x0100,
    RxTxTimeout = 0x0200,
    All = 0xFFFF,
}

impl IrqMask {
    pub fn value(self) -> u16 {
        self as u16
    }

    pub fn is_set_in(self, mask: u16) -> bool {
        self.value() & mask == self.value()
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub enum Register {
    PacketParams = 0x0704,  // packet configuration
    PayloadLength = 0x0702, // payload size
    /// Number of symbols given as SX126X_REG_LR_SYNCH_TIMEOUT[7:3] * 2 ^ (2*SX126X_REG_LR_SYNCH_TIMEOUT[2:0] + 1)
    /// Info from SDK (not present in user manual).
    SynchTimeout = 0x0706,
    Syncword = 0x06C0,              // Syncword values
    LoRaSyncword = 0x0740,          // LoRa Syncword value
    GeneratedRandomNumber = 0x0819, //32-bit generated random number
    AnaLNA = 0x08E2,                // disable the LNA
    AnaMixer = 0x08E5,              // disable the mixer
    RxGain = 0x08AC,                // RX gain (0x94: power saving, 0x96: rx boosted)
    XTATrim = 0x0911,               // device internal trimming capacitor
    OCP = 0x08E7,                   // over current protection max value
    RetentionList = 0x029F,         // retention list
    /// Inverted IQ operation optimization - possible packet loss for longer packets
    /// DS.SX1261-2.W.APP Rev.2.1 - Chapter 15.4
    IQPolarity = 0x0736,
    TxModulation = 0x0889, // modulation quality with 500 kHz LoRa Bandwidth (see DS_SX1261-2_V1.2 datasheet chapter 15.1)
    TxClampCfg = 0x08D8,   // better resistance to antenna mismatch (see DS_SX1261-2_V1.2 datasheet chapter 15.2)
    RTCCtrl = 0x0902,      // RTC control
    EvtClr = 0x0944,       // event clear
}

impl Register {
    pub fn addr1(self) -> u8 {
        ((self as u16 & 0xFF00) >> 8) as u8
    }
    pub fn addr2(self) -> u8 {
        (self as u16 & 0x00FF) as u8
    }
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum OpCode {
    GetStatus = 0xC0,
    WriteRegister = 0x0D,
    ReadRegister = 0x1D,
    WriteBuffer = 0x0E,
    ReadBuffer = 0x1E,
    SetSleep = 0x84,
    SetStandby = 0x80,
    SetFS = 0xC1,
    SetTx = 0x83,
    SetRx = 0x82,
    SetRxDutyCycle = 0x94,
    SetCAD = 0xC5,
    SetTxContinuousWave = 0xD1,
    SetTxContinuousPremable = 0xD2,
    SetPacketType = 0x8A,
    GetPacketType = 0x11,
    SetRFFrequency = 0x86,
    SetTxParams = 0x8E,
    SetPAConfig = 0x95,
    SetCADParams = 0x88,
    SetBufferBaseAddress = 0x8F,
    SetModulationParams = 0x8B,
    SetPacketParams = 0x8C,
    GetRxBufferStatus = 0x13,
    GetPacketStatus = 0x14,
    GetRSSIInst = 0x15,
    GetStats = 0x10,
    ResetStats = 0x00,
    CfgDIOIrq = 0x08,
    GetIrqStatus = 0x12,
    ClrIrqStatus = 0x02,
    Calibrate = 0x89,
    CalibrateImage = 0x98,
    SetRegulatorMode = 0x96,
    GetDeviceErrors = 0x17,
    ClearDeviceErrors = 0x07,
    SetTCXOMode = 0x97,
    SetTxFallbackMode = 0x93,
    SetDIO2AsRfSwitchCtrl = 0x9d,
    SetStopRxTimerOnPreamble = 0x9F,
    SetLoRaSymbTimeout = 0xA0,
}

impl OpCode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

// See RM0453 Reference manual STM32WL5x advanced Arm®-based 32-bit MCUs with sub-GHz radio solution, section 5.8.5
#[derive(Clone, Copy, PartialEq)]
pub enum OpStatusErrorMask {
    Timeout = (0x03 << 1),
    ProcessingError = (0x04 << 1),
    ExecutionError = (0x05 << 1),
}

impl OpStatusErrorMask {
    pub fn is_error(status: u8) -> bool {
        let error_flags = status & 0x0e;
        OpStatusErrorMask::Timeout as u8 == error_flags
            || OpStatusErrorMask::ProcessingError as u8 == error_flags
            || OpStatusErrorMask::ExecutionError as u8 == error_flags
    }
}

#[derive(Clone, Copy)]
pub struct SleepParams {
    pub wakeup_rtc: bool, // get out of sleep mode if wakeup signal received from RTC
    pub reset: bool,
    pub warm_start: bool,
}

impl SleepParams {
    pub fn value(self) -> u8 {
        ((self.warm_start as u8) << 2) | ((self.reset as u8) << 1) | (self.wakeup_rtc as u8)
    }
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub enum StandbyMode {
    RC = 0x00,
    XOSC = 0x01,
}

impl StandbyMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum RegulatorMode {
    UseLDO = 0x00,
    UseDCDC = 0x01,
}

impl RegulatorMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct CalibrationParams {
    pub rc64k_enable: bool,     // calibrate RC64K clock
    pub rc13m_enable: bool,     // calibrate RC13M clock
    pub pll_enable: bool,       // calibrate PLL
    pub adc_pulse_enable: bool, // calibrate ADC Pulse
    pub adc_bulkn_enable: bool, // calibrate ADC bulkN
    pub adc_bulkp_enable: bool, // calibrate ADC bulkP
    pub img_enable: bool,
}

#[allow(dead_code)]
impl CalibrationParams {
    pub fn value(self) -> u8 {
        ((self.img_enable as u8) << 6)
            | ((self.adc_bulkp_enable as u8) << 5)
            | ((self.adc_bulkn_enable as u8) << 4)
            | ((self.adc_pulse_enable as u8) << 3)
            | ((self.pll_enable as u8) << 2)
            | ((self.rc13m_enable as u8) << 1)
            | (self.rc64k_enable as u8)
    }
}

#[allow(missing_docs)]
#[derive(Clone, Copy)]
pub enum TcxoCtrlVoltage {
    Ctrl1V6 = 0x00,
    Ctrl1V7 = 0x01,
    Ctrl1V8 = 0x02,
    Ctrl2V2 = 0x03,
    Ctrl2V4 = 0x04,
    Ctrl2V7 = 0x05,
    Ctrl3V0 = 0x06,
    Ctrl3V3 = 0x07,
}

#[allow(missing_docs)]
impl TcxoCtrlVoltage {
    pub fn value(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
#[allow(clippy::enum_variant_names)]
pub enum RampTime {
    Ramp10Us = 0x00,
    Ramp20Us = 0x01,
    Ramp40Us = 0x02,
    Ramp80Us = 0x03,
    Ramp200Us = 0x04,
    Ramp800Us = 0x05,
    Ramp1700Us = 0x06,
    Ramp3400Us = 0x07,
}

impl RampTime {
    pub fn value(self) -> u8 {
        self as u8
    }
}

pub fn spreading_factor_value(spreading_factor: SpreadingFactor) -> Result<u8, RadioError> {
    match spreading_factor {
        SpreadingFactor::_5 => Ok(0x05),
        SpreadingFactor::_6 => Ok(0x06),
        SpreadingFactor::_7 => Ok(0x07),
        SpreadingFactor::_8 => Ok(0x08),
        SpreadingFactor::_9 => Ok(0x09),
        SpreadingFactor::_10 => Ok(0x0A),
        SpreadingFactor::_11 => Ok(0x0B),
        SpreadingFactor::_12 => Ok(0x0C),
    }
}

pub fn bandwidth_value(bandwidth: Bandwidth) -> Result<u8, RadioError> {
    match bandwidth {
        Bandwidth::_7KHz => Ok(0x00),
        Bandwidth::_10KHz => Ok(0x08),
        Bandwidth::_15KHz => Ok(0x01),
        Bandwidth::_20KHz => Ok(0x09),
        Bandwidth::_31KHz => Ok(0x02),
        Bandwidth::_41KHz => Ok(0x0a),
        Bandwidth::_62KHz => Ok(0x03),
        Bandwidth::_125KHz => Ok(0x04),
        Bandwidth::_250KHz => Ok(0x05),
        Bandwidth::_500KHz => Ok(0x06),
    }
}

pub fn coding_rate_value(coding_rate: CodingRate) -> Result<u8, RadioError> {
    match coding_rate {
        CodingRate::_4_5 => Ok(0x01),
        CodingRate::_4_6 => Ok(0x02),
        CodingRate::_4_7 => Ok(0x03),
        CodingRate::_4_8 => Ok(0x04),
    }
}

#[derive(Clone, Copy)]
pub enum CADSymbols {
    _1 = 0x00,
    _2 = 0x01,
    _4 = 0x02,
    _8 = 0x03,
    _16 = 0x04,
}

impl CADSymbols {
    pub fn value(self) -> u8 {
        self as u8
    }
}
