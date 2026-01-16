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
    /// Write with mask (read-modify-write) - used for High ACP workaround
    WriteRegMem32Mask = 0x010C,
}

/// Register address for High ACP workaround (from SWDR001)
pub const HIGH_ACP_WORKAROUND_REG: u32 = 0x00F30054;

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

// =============================================================================
// LR-FHSS Types and Parameters
// =============================================================================

/// LR-FHSS OpCodes (16-bit commands)
#[derive(Clone, Copy, PartialEq)]
pub enum LrFhssOpCode {
    Init = 0x022C,
    BuildFrame = 0x022D,
    SetSyncWord = 0x022E,
}

impl LrFhssOpCode {
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// LR-FHSS modulation type
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum LrFhssModulationType {
    Gmsk488 = 0x00,
}

impl LrFhssModulationType {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// LR-FHSS coding rate
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum LrFhssCodingRate {
    Cr5_6 = 0x00,
    Cr2_3 = 0x01,
    Cr1_2 = 0x02,
    Cr1_3 = 0x03,
}

impl LrFhssCodingRate {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// LR-FHSS grid spacing
/// Note: Values match lr_fhss_v1_grid_t from SWDM001/SWDR001
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum LrFhssGrid {
    /// 25.391 kHz grid (coarse)
    Grid25391Hz = 0x00,
    /// 3.906 kHz grid (fine)
    Grid3906Hz = 0x01,
}

impl LrFhssGrid {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// LR-FHSS bandwidth
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum LrFhssBandwidth {
    Bw39063Hz = 0x00,
    Bw85938Hz = 0x01,
    Bw136719Hz = 0x02,
    Bw183594Hz = 0x03,
    Bw335938Hz = 0x04,
    Bw386719Hz = 0x05,
    Bw722656Hz = 0x06,
    Bw773438Hz = 0x07,
    Bw1523438Hz = 0x08,
    Bw1574219Hz = 0x09,
}

impl LrFhssBandwidth {
    pub fn value(self) -> u8 {
        self as u8
    }

    /// Get the number of hop sequences for this bandwidth and grid
    /// Values from SWDM001 lr_fhss_v1_base_types.h
    pub fn hop_sequence_count(self, grid: LrFhssGrid) -> u16 {
        match grid {
            LrFhssGrid::Grid25391Hz => match self {
                LrFhssBandwidth::Bw39063Hz => 1,
                LrFhssBandwidth::Bw85938Hz => 1,
                LrFhssBandwidth::Bw136719Hz => 1,
                LrFhssBandwidth::Bw183594Hz => 1,
                LrFhssBandwidth::Bw335938Hz => 44,
                LrFhssBandwidth::Bw386719Hz => 50,
                LrFhssBandwidth::Bw722656Hz => 88,
                LrFhssBandwidth::Bw773438Hz => 94,
                LrFhssBandwidth::Bw1523438Hz => 176,
                LrFhssBandwidth::Bw1574219Hz => 182,
            },
            LrFhssGrid::Grid3906Hz => match self {
                LrFhssBandwidth::Bw39063Hz => 1,
                LrFhssBandwidth::Bw85938Hz => 85,
                LrFhssBandwidth::Bw136719Hz => 170,
                LrFhssBandwidth::Bw183594Hz => 255,
                LrFhssBandwidth::Bw335938Hz => 340,
                LrFhssBandwidth::Bw386719Hz => 383,
                LrFhssBandwidth::Bw722656Hz => 639,
                LrFhssBandwidth::Bw773438Hz => 682,
                LrFhssBandwidth::Bw1523438Hz => 1192,
                LrFhssBandwidth::Bw1574219Hz => 1235,
            },
        }
    }
}

/// Default LR-FHSS sync word from SWDM001: { 0x2C, 0x0F, 0x79, 0x95 }
pub const LR_FHSS_DEFAULT_SYNC_WORD: [u8; LR_FHSS_SYNC_WORD_BYTES] = [0x2C, 0x0F, 0x79, 0x95];

/// LR-FHSS V1 parameters (matching lr_fhss_v1_params_t from SWDM001/SWDR001)
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct LrFhssV1Params {
    /// 4-byte sync word (default: 0x2C, 0x0F, 0x79, 0x95)
    pub sync_word: [u8; LR_FHSS_SYNC_WORD_BYTES],
    /// Modulation type (GMSK 488 bps)
    pub modulation_type: LrFhssModulationType,
    /// Coding rate
    pub coding_rate: LrFhssCodingRate,
    /// Grid spacing
    pub grid: LrFhssGrid,
    /// Enable frequency hopping
    pub enable_hopping: bool,
    /// Bandwidth
    pub bandwidth: LrFhssBandwidth,
    /// Number of header blocks
    pub header_count: u8,
}

/// LR-FHSS parameters (matching lr11xx_lr_fhss_params_t from SWDR001)
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct LrFhssParams {
    pub lr_fhss_params: LrFhssV1Params,
    pub device_offset: i8,
}

/// LR-FHSS sync word bytes
pub const LR_FHSS_SYNC_WORD_BYTES: usize = 4;

// =============================================================================
// System Types (from SWDR001 lr11xx_system_types.h)
// =============================================================================

/// Length of the LR11XX Unique Identifier in bytes
pub const LR11XX_SYSTEM_UID_LENGTH: usize = 8;

/// Length of the LR11XX Join EUI in bytes
pub const LR11XX_SYSTEM_JOIN_EUI_LENGTH: usize = 8;

/// Chip type/version values
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum ChipType {
    Lr1110 = 0x01,
    Lr1120 = 0x02,
    Lr1121 = 0x03,
    Unknown = 0xFF,
}

impl From<u8> for ChipType {
    fn from(value: u8) -> Self {
        match value {
            0x01 => ChipType::Lr1110,
            0x02 => ChipType::Lr1120,
            0x03 => ChipType::Lr1121,
            _ => ChipType::Unknown,
        }
    }
}

/// System version information
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct Version {
    /// Hardware version
    pub hw: u8,
    /// Chip type (LR1110, LR1120, LR1121)
    pub chip_type: ChipType,
    /// Firmware version (major.minor encoded as u16)
    pub fw: u16,
}

impl Version {
    /// Get firmware major version
    pub fn fw_major(&self) -> u8 {
        (self.fw >> 8) as u8
    }

    /// Get firmware minor version
    pub fn fw_minor(&self) -> u8 {
        (self.fw & 0xFF) as u8
    }
}

/// Chip operating modes
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum ChipMode {
    Sleep = 0x00,
    StandbyRc = 0x01,
    StandbyXosc = 0x02,
    Fs = 0x03,
    Rx = 0x04,
    Tx = 0x05,
    Loc = 0x06,  // GNSS/WiFi scanning
    Unknown = 0xFF,
}

impl From<u8> for ChipMode {
    fn from(value: u8) -> Self {
        match value {
            0x00 => ChipMode::Sleep,
            0x01 => ChipMode::StandbyRc,
            0x02 => ChipMode::StandbyXosc,
            0x03 => ChipMode::Fs,
            0x04 => ChipMode::Rx,
            0x05 => ChipMode::Tx,
            0x06 => ChipMode::Loc,
            _ => ChipMode::Unknown,
        }
    }
}

/// Reset status values
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum ResetStatus {
    Cleared = 0x00,
    Analog = 0x01,
    External = 0x02,
    System = 0x03,
    Watchdog = 0x04,
    IocdRestart = 0x05,
    RtcRestart = 0x06,
    Unknown = 0xFF,
}

impl From<u8> for ResetStatus {
    fn from(value: u8) -> Self {
        match value {
            0x00 => ResetStatus::Cleared,
            0x01 => ResetStatus::Analog,
            0x02 => ResetStatus::External,
            0x03 => ResetStatus::System,
            0x04 => ResetStatus::Watchdog,
            0x05 => ResetStatus::IocdRestart,
            0x06 => ResetStatus::RtcRestart,
            _ => ResetStatus::Unknown,
        }
    }
}

/// Command status values
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum CommandStatus {
    Fail = 0x00,
    PeripheralError = 0x01,
    Ok = 0x02,
    Data = 0x03,
}

impl From<u8> for CommandStatus {
    fn from(value: u8) -> Self {
        match value {
            0x00 => CommandStatus::Fail,
            0x01 => CommandStatus::PeripheralError,
            0x02 => CommandStatus::Ok,
            0x03 => CommandStatus::Data,
            _ => CommandStatus::Fail,
        }
    }
}

/// Status register 1
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct Stat1 {
    /// Command status
    pub command_status: CommandStatus,
    /// Whether an interrupt is currently active
    pub is_interrupt_active: bool,
}

impl From<u8> for Stat1 {
    fn from(value: u8) -> Self {
        Self {
            is_interrupt_active: (value & 0x01) != 0,
            command_status: CommandStatus::from(value >> 1),
        }
    }
}

/// Status register 2
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct Stat2 {
    /// Reset status
    pub reset_status: ResetStatus,
    /// Current chip mode
    pub chip_mode: ChipMode,
    /// Whether running from flash
    pub is_running_from_flash: bool,
}

impl From<u8> for Stat2 {
    fn from(value: u8) -> Self {
        Self {
            is_running_from_flash: (value & 0x01) != 0,
            chip_mode: ChipMode::from((value & 0x0E) >> 1),
            reset_status: ResetStatus::from((value & 0xF0) >> 4),
        }
    }
}

/// Combined system status
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct SystemStatus {
    pub stat1: Stat1,
    pub stat2: Stat2,
    pub irq_status: u32,
}
