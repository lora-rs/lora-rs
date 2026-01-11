/// LR1110 radio driver parameter types and constants
///
/// This module defines all the enums, structs, and constants used by the LR1110 radio driver.
/// Implementation is based on the official SWDR001 C driver.

use crate::mod_params::*;

/// LR1110 crystal frequency (32 MHz)
pub const LR1110_XTAL_FREQ_HZ: u32 = 32_000_000;

/// PLL step shift amount for frequency calculations
pub const LR1110_PLL_STEP_SHIFT: u32 = 14;

/// Internal RTC frequency
pub const LR1110_RTC_FREQ_HZ: u32 = 32768;

/// Packet types supported by LR1110
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum PacketType {
    None = 0x00,
    Gfsk = 0x01,
    LoRa = 0x02,
    Bpsk = 0x03,
    LrFhss = 0x04,
    Rttof = 0x05,
}

impl PacketType {
    pub const fn value(self) -> u8 {
        self as u8
    }
}

/// IRQ flags for LR1110 (32-bit mask)
/// Note: LR1110 uses 32-bit IRQ masks unlike SX126x which uses 16-bit
#[derive(Clone, Copy)]
pub enum IrqMask {
    None = 0x00000000,
    TxDone = 0x00000004,                   // bit 2
    RxDone = 0x00000008,                   // bit 3
    PreambleDetected = 0x00000010,         // bit 4
    SyncWordHeaderValid = 0x00000020,      // bit 5
    HeaderError = 0x00000040,              // bit 6
    CrcError = 0x00000080,                 // bit 7
    CadDone = 0x00000100,                  // bit 8
    CadDetected = 0x00000200,              // bit 9
    Timeout = 0x00000400,                  // bit 10
    LrFhssIntraPktHop = 0x00000800,        // bit 11
    RttofReqValid = 0x00004000,            // bit 14
    RttofReqDiscarded = 0x00008000,        // bit 15
    RttofRespDone = 0x00010000,            // bit 16
    RttofExchValid = 0x00020000,           // bit 17
    RttofTimeout = 0x00040000,             // bit 18
    GnssScanDone = 0x00080000,             // bit 19
    WifiScanDone = 0x00100000,             // bit 20
    Eol = 0x00200000,                      // bit 21
    CmdError = 0x00400000,                 // bit 22
    Error = 0x00800000,                    // bit 23
    FskLenError = 0x01000000,              // bit 24
    FskAddrError = 0x02000000,             // bit 25
    LoRaRxTimestamp = 0x08000000,          // bit 27
}

impl IrqMask {
    pub fn value(self) -> u32 {
        self as u32
    }

    pub fn is_set(self, mask: u32) -> bool {
        self.value() & mask == self.value()
    }
}

/// System OpCodes (16-bit commands for LR1110)
#[derive(Clone, Copy, PartialEq)]
pub enum SystemOpCode {
    GetStatus = 0x0100,
    GetVersion = 0x0101,
    GetErrors = 0x010D,
    ClearErrors = 0x010E,
    Calibrate = 0x010F,
    SetRegMode = 0x0110,
    CalibrateImage = 0x0111,
    SetDioAsRfSwitch = 0x0112,
    SetDioIrqParams = 0x0113,
    ClearIrq = 0x0114,
    CfgLfClk = 0x0116,
    SetTcxoMode = 0x0117,
    Reboot = 0x0118,
    GetVbat = 0x0119,
    GetTemp = 0x011A,
    SetSleep = 0x011B,
    SetStandby = 0x011C,
    SetFs = 0x011D,
    GetRandom = 0x0120,
    EraseInfoPage = 0x0121,
    WriteInfoPage = 0x0122,
    ReadInfoPage = 0x0123,
    ReadUid = 0x0125,
    ReadJoinEui = 0x0126,
    ReadPin = 0x0127,
    EnableSpiCrc = 0x0128,
    DriveDioInSleepMode = 0x012A,
}

impl SystemOpCode {
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// Radio OpCodes (16-bit commands for LR1110)
#[derive(Clone, Copy, PartialEq)]
pub enum RadioOpCode {
    ResetStats = 0x0200,
    GetStats = 0x0201,
    GetPktType = 0x0202,
    GetRxBufferStatus = 0x0203,
    GetPktStatus = 0x0204,
    GetRssiInst = 0x0205,
    SetGfskSyncWord = 0x0206,
    SetLoRaPublicNetwork = 0x0208,
    SetRx = 0x0209,
    SetTx = 0x020A,
    SetRfFrequency = 0x020B,
    AutoTxRx = 0x020C,
    SetCadParams = 0x020D,
    SetPktType = 0x020E,
    SetModulationParam = 0x020F,
    SetPktParam = 0x0210,
    SetTxParams = 0x0211,
    SetPktAdrs = 0x0212,
    SetRxTxFallbackMode = 0x0213,
    SetRxDutyCycle = 0x0214,
    SetPaCfg = 0x0215,
    StopTimeoutOnPreamble = 0x0217,
    SetCad = 0x0218,
    SetTxCw = 0x0219,
    SetTxInfinitePreamble = 0x021A,
    SetLoRaSyncTimeout = 0x021B,
    SetGfskCrcParams = 0x0224,
    SetGfskWhiteningParams = 0x0225,
    SetRxBoosted = 0x0227,
    SetRssiCalibration = 0x0229,
    SetLoRaSyncWord = 0x022B,
    SetLrFhssSyncWord = 0x022D,
    CfgBluetoothLowEnergyBeaconningCompatibility = 0x022E,
    GetLoRaRxInfo = 0x0230,
    BluetoothLowEnergyBeaconningCompatibilitySend = 0x0231,
}

impl RadioOpCode {
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// Register/Memory OpCodes
#[derive(Clone, Copy, PartialEq)]
pub enum RegMemOpCode {
    WriteRegMem = 0x0105,
    ReadRegMem = 0x0106,
    WriteBuffer8 = 0x0108,
    ReadBuffer8 = 0x0109,
}

impl RegMemOpCode {
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// Standby modes
#[derive(Clone, Copy, PartialEq)]
pub enum StandbyMode {
    Rc = 0x00,
    Xosc = 0x01,
}

impl StandbyMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Regulator mode
#[derive(Clone, Copy)]
pub enum RegulatorMode {
    Ldo = 0x00,
    Dcdc = 0x01,
}

impl RegulatorMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// TCXO control voltage
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

impl TcxoCtrlVoltage {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Power Amplifier selection
#[derive(Clone, Copy, PartialEq)]
pub enum PaSelection {
    Lp = 0x00,  // Low-power PA (up to +14dBm)
    Hp = 0x01,  // High-power PA (up to +22dBm)
    Hf = 0x02,  // High-frequency PA (2.4GHz)
}

impl PaSelection {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Power Amplifier regulator supply
#[derive(Clone, Copy)]
pub enum PaRegSupply {
    Vreg = 0x00,  // From internal regulator
    Vbat = 0x01,  // From battery
}

impl PaRegSupply {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Ramp time for PA
#[derive(Clone, Copy)]
pub enum RampTime {
    Ramp16Us = 0x00,
    Ramp32Us = 0x01,
    Ramp48Us = 0x02,
    Ramp64Us = 0x03,
    Ramp80Us = 0x04,
    Ramp96Us = 0x05,
    Ramp112Us = 0x06,
    Ramp128Us = 0x07,
    Ramp144Us = 0x08,
    Ramp160Us = 0x09,
    Ramp176Us = 0x0A,
    Ramp192Us = 0x0B,
    Ramp208Us = 0x0C,
    Ramp240Us = 0x0D,
    Ramp272Us = 0x0E,
    Ramp304Us = 0x0F,
}

impl RampTime {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// LoRa spreading factor
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

/// LoRa bandwidth values for LR1110
pub fn bandwidth_value(bandwidth: Bandwidth) -> Result<u8, RadioError> {
    match bandwidth {
        Bandwidth::_7KHz => Err(RadioError::InvalidBandwidthForFrequency),  // Not supported on LR1110
        Bandwidth::_10KHz => Ok(0x08),
        Bandwidth::_15KHz => Ok(0x01),
        Bandwidth::_20KHz => Ok(0x09),
        Bandwidth::_31KHz => Ok(0x02),
        Bandwidth::_41KHz => Ok(0x0A),
        Bandwidth::_62KHz => Ok(0x03),
        Bandwidth::_125KHz => Ok(0x04),
        Bandwidth::_250KHz => Ok(0x05),
        Bandwidth::_500KHz => Ok(0x06),
    }
}

/// LoRa coding rate
pub fn coding_rate_value(coding_rate: CodingRate) -> Result<u8, RadioError> {
    match coding_rate {
        CodingRate::_4_5 => Ok(0x01),
        CodingRate::_4_6 => Ok(0x02),
        CodingRate::_4_7 => Ok(0x03),
        CodingRate::_4_8 => Ok(0x04),
    }
}

/// CAD (Channel Activity Detection) symbols
#[derive(Clone, Copy)]
pub enum CadSymbols {
    _1 = 0x00,
    _2 = 0x01,
    _4 = 0x02,
    _8 = 0x03,
    _16 = 0x04,
}

impl CadSymbols {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// CAD exit mode
#[derive(Clone, Copy)]
pub enum CadExitMode {
    StandbyRc = 0x00,
    Rx = 0x01,
    Tx = 0x10,
}

impl CadExitMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// LoRa CRC configuration
#[derive(Clone, Copy)]
pub enum LoRaCrc {
    Off = 0x00,
    On = 0x01,
}

impl LoRaCrc {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// LoRa header type
#[derive(Clone, Copy)]
pub enum LoRaHeaderType {
    Explicit = 0x00,
    Implicit = 0x01,
}

impl LoRaHeaderType {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// LoRa IQ configuration
#[derive(Clone, Copy)]
pub enum LoRaIq {
    Standard = 0x00,
    Inverted = 0x01,
}

impl LoRaIq {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Fallback mode after TX/RX
#[derive(Clone, Copy)]
pub enum FallbackMode {
    StandbyRc = 0x01,
    StandbyXosc = 0x02,
    Fs = 0x03,
}

impl FallbackMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Sleep configuration
pub struct SleepParams {
    pub warm_start: bool,
    pub rtc_wakeup: bool,
}

impl SleepParams {
    pub fn value(self) -> u8 {
        let mut val = 0u8;
        if self.warm_start {
            val |= 0x04;
        }
        if self.rtc_wakeup {
            val |= 0x01;
        }
        val
    }
}

/// Calibration parameters
pub struct CalibrationParams {
    pub rc64k_enable: bool,
    pub rc13m_enable: bool,
    pub pll_enable: bool,
    pub adc_pulse_enable: bool,
    pub adc_bulkn_enable: bool,
    pub adc_bulkp_enable: bool,
    pub img_enable: bool,
}

impl CalibrationParams {
    pub fn value(self) -> u8 {
        ((self.img_enable as u8) << 4)
            | ((self.adc_bulkp_enable as u8) << 3)
            | ((self.adc_bulkn_enable as u8) << 2)
            | ((self.adc_pulse_enable as u8) << 1)
            | ((self.pll_enable as u8) << 0)
    }
}

/// Convert frequency in Hz to PLL step value
pub fn convert_freq_in_hz_to_pll_step(freq_in_hz: u32) -> u32 {
    // freq_in_hz * 2^25 / 32MHz = freq_in_hz * (1 << 25) / 32000000
    // Simplify: freq_in_hz * 2^25 / (2^5 * 10^6) = freq_in_hz * 2^20 / 10^6
    // Or use: (freq_in_hz << 14) / 32000000 * (1 << 11)
    //
    // Formula from SWDR001: freq_in_hz * (1 << 25) / LR1110_XTAL_FREQ_HZ
    let freq_in_pll_steps = ((freq_in_hz as u64) << 25) / (LR1110_XTAL_FREQ_HZ as u64);
    freq_in_pll_steps as u32
}

/// Convert time in milliseconds to RTC steps
pub fn convert_time_in_ms_to_rtc_step(time_in_ms: u32) -> u32 {
    // RTC runs at 32.768 kHz, so 1ms = 32.768 ticks
    // time_in_ms * 32768 / 1000 = time_in_ms * 32.768
    ((time_in_ms as u64 * LR1110_RTC_FREQ_HZ as u64) / 1000) as u32
}
