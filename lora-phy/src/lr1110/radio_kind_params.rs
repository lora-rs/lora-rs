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
    /// Get the raw u32 value of this IRQ mask
    pub const fn value(self) -> u32 {
        self as u32
    }

    /// Check if this IRQ flag is set in the given mask
    pub fn is_set(self, mask: u32) -> bool {
        (self as u32) & mask == (self as u32)
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

/// Register/Memory OpCodes (from SWDR001 lr11xx_regmem.c)
#[derive(Clone, Copy, PartialEq)]
pub enum RegMemOpCode {
    /// Write 32-bit words to register/memory (0x0105)
    WriteRegMem32 = 0x0105,
    /// Read 32-bit words from register/memory (0x0106)
    ReadRegMem32 = 0x0106,
    /// Write bytes to memory (0x0107)
    WriteMem8 = 0x0107,
    /// Read bytes from memory (0x0108)
    ReadMem8 = 0x0108,
    /// Write bytes to TX buffer (0x0109)
    WriteBuffer8 = 0x0109,
    /// Read bytes from RX buffer (0x010A)
    ReadBuffer8 = 0x010A,
    /// Clear RX buffer (0x010B)
    ClearRxBuffer = 0x010B,
    /// Write with mask (read-modify-write) (0x010C)
    WriteRegMem32Mask = 0x010C,
}

impl RegMemOpCode {
    /// Convert opcode to bytes for SPI command
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// Maximum number of 32-bit words for single read/write operation
pub const REGMEM_MAX_READ_WRITE_WORDS: usize = 64;

/// Maximum buffer size in bytes
pub const REGMEM_BUFFER_SIZE_MAX: usize = 256;

/// Register address for High ACP workaround (from SWDR001)
pub const HIGH_ACP_WORKAROUND_REG: u32 = 0x00F30054;

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
/// Note: LR-FHSS init is done via SetPacketType + SetLrFhssModParams, not a single opcode.
/// Sync word is set via RadioOpCode::SetLrFhssSyncWord (0x022D).
#[derive(Clone, Copy, PartialEq)]
pub enum LrFhssOpCode {
    /// Build LR-FHSS frame (0x022C) - the main LR-FHSS command
    BuildFrame = 0x022C,
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

// =============================================================================
// GNSS Types and Constants (from SWDR001 lr11xx_gnss.c and lr11xx_gnss_types.h)
// =============================================================================

/// GNSS OpCodes (16-bit commands)
#[derive(Clone, Copy, PartialEq)]
pub enum GnssOpCode {
    /// Set the constellation to use (0x0400)
    SetConstellation = 0x0400,
    /// Read the used constellations (0x0401)
    ReadConstellation = 0x0401,
    /// Set almanac update configuration (0x0402)
    SetAlmanacUpdate = 0x0402,
    /// Read the almanac update configuration (0x0403)
    ReadAlmanacUpdate = 0x0403,
    /// Set the frequency search space (0x0404)
    SetFreqSearchSpace = 0x0404,
    /// Read the frequency search space (0x0405)
    ReadFreqSearchSpace = 0x0405,
    /// Read the GNSS firmware version (0x0406)
    ReadFwVersion = 0x0406,
    /// Read the supported constellations (0x0407)
    ReadSupportedConstellation = 0x0407,
    /// Define single or double capture mode (0x0408)
    SetScanMode = 0x0408,
    /// Launch the scan (0x040B)
    Scan = 0x040B,
    /// Get the size of the output payload (0x040C)
    GetResultSize = 0x040C,
    /// Read the result byte stream (0x040D)
    ReadResults = 0x040D,
    /// Update the almanac (0x040E)
    AlmanacUpdate = 0x040E,
    /// Read all almanacs (0x040F)
    AlmanacRead = 0x040F,
    /// Set the assistance position (0x0410)
    SetAssistancePosition = 0x0410,
    /// Read the assistance position (0x0411)
    ReadAssistancePosition = 0x0411,
    /// Push messages coming from the solver (0x0414)
    PushSolverMsg = 0x0414,
    /// Push messages coming from the device management (0x0415)
    PushDmMsg = 0x0415,
    /// Read the context (0x0416)
    GetContextStatus = 0x0416,
    /// Get the number of satellites detected during a scan (0x0417)
    GetNbSatellites = 0x0417,
    /// Get the list of satellites detected during a scan (0x0418)
    GetSatellites = 0x0418,
    /// Read the almanac of given satellites (0x041A)
    ReadAlmanacPerSatellite = 0x041A,
    /// Get the number of visible SV from a date and position (0x041F)
    GetSvVisible = 0x041F,
    /// Get visible SV ID and corresponding doppler value (0x0420)
    GetSvVisibleDoppler = 0x0420,
    /// Get the type of scan launched during the last scan (0x0426)
    ReadLastScanModeLaunched = 0x0426,
    /// Start the time acquisition/demodulation (0x0432)
    FetchTime = 0x0432,
    /// Read time from LR11XX (0x0434)
    ReadTime = 0x0434,
    /// Reset the internal time (0x0435)
    ResetTime = 0x0435,
    /// Reset the location and the history Doppler buffer (0x0437)
    ResetPosition = 0x0437,
    /// Read the week number rollover (0x0438)
    ReadWeekNumberRollover = 0x0438,
    /// Read demod status (0x0439)
    ReadDemodStatus = 0x0439,
    /// Read cumulative timing (0x044A)
    ReadCumulativeTiming = 0x044A,
    /// Set the GPS time (0x044B)
    SetTime = 0x044B,
    /// Configures the time delay in sec (0x044D)
    ConfigDelayResetAp = 0x044D,
    /// Read the assisted position based on the internal doppler solver (0x044F)
    ReadDopplerSolverResult = 0x044F,
    /// Read the time delay in sec (0x0453)
    ReadDelayResetAp = 0x0453,
    /// Launches one scan to download from satellite almanac parameters broadcasted (0x0454)
    AlmanacUpdateFromSat = 0x0454,
    /// Read the number of visible satellites and time elapsed (0x0456)
    ReadKeepSyncStatus = 0x0456,
    /// Returns the actual state of almanac GPS and Beidou (0x0457)
    ReadAlmanacStatus = 0x0457,
    /// Configures the almanac update period (0x0463)
    ConfigAlmanacUpdatePeriod = 0x0463,
    /// Read the almanac update period (0x0464)
    ReadAlmanacUpdatePeriod = 0x0464,
    /// Returns the list of satellite for the next keep sync scan (0x0466)
    GetSvSync = 0x0466,
    /// Configures the ability to search almanac for each satellite (0x0472)
    WriteBitMaskSatActivated = 0x0472,
}

impl GnssOpCode {
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// GNSS constellation identifiers
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GnssConstellation {
    /// GPS constellation
    Gps = 0x01,
    /// BeiDou constellation
    BeiDou = 0x02,
}

impl GnssConstellation {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Bit mask of constellation configurations
pub type GnssConstellationMask = u8;

/// GPS constellation mask
pub const GNSS_GPS_MASK: GnssConstellationMask = 0x01;
/// BeiDou constellation mask
pub const GNSS_BEIDOU_MASK: GnssConstellationMask = 0x02;

/// Search mode for GNSS scan
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GnssSearchMode {
    /// Search all requested satellites or fail, scan duration is low
    LowEffort = 0x00,
    /// Add additional search if not all satellites are found, scan duration is standard
    MidEffort = 0x01,
    /// Add additional search if not all satellites are found, scan duration is very high
    HighEffort = 0x02,
}

impl GnssSearchMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// GNSS response type indicates the destination
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GnssDestination {
    /// Host MCU
    Host = 0x00,
    /// GNSS Solver (LoRa Cloud)
    Solver = 0x01,
    /// GNSS DMC (Device Management Component)
    Dmc = 0x02,
}

impl GnssDestination {
    pub fn value(self) -> u8 {
        self as u8
    }
}

impl From<u8> for GnssDestination {
    fn from(value: u8) -> Self {
        match value {
            0x00 => GnssDestination::Host,
            0x01 => GnssDestination::Solver,
            0x02 => GnssDestination::Dmc,
            _ => GnssDestination::Host,
        }
    }
}

/// GNSS single or double scan mode
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GnssScanMode {
    /// Single scan legacy mode - NAV3 format
    SingleScanLegacy = 0x00,
    /// Single scan and 5 fast scans - NAV3 format
    SingleScanAnd5FastScans = 0x03,
}

impl GnssScanMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Message to host indicating the status of the message
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GnssHostStatus {
    Ok = 0x00,
    UnexpectedCmd = 0x01,
    UnimplementedCmd = 0x02,
    InvalidParameters = 0x03,
    MessageSanityCheckError = 0x04,
    IqCaptureFails = 0x05,
    NoTime = 0x06,
    NoSatelliteDetected = 0x07,
    AlmanacInFlashTooOld = 0x08,
    AlmanacUpdateFailsCrcError = 0x09,
    AlmanacUpdateFailsFlashIntegrityError = 0x0A,
    AlmanacUpdateNotAllowed = 0x0C,
    AlmanacCrcError = 0x0D,
    AlmanacVersionNotSupported = 0x0E,
    NotEnoughSvDetectedToBuildNavMessage = 0x10,
    TimeDemodulationFail = 0x11,
    AlmanacDemodulationFail = 0x12,
    AtLeastTheDetectedSvOfOneConstellationAreDeactivated = 0x13,
    AssistancePositionPossiblyWrongButFailsToUpdate = 0x14,
    ScanAborted = 0x15,
    NavMessageCannotBeGeneratedIntervalGreaterThan63Sec = 0x16,
}

impl From<u8> for GnssHostStatus {
    fn from(value: u8) -> Self {
        match value {
            0x00 => GnssHostStatus::Ok,
            0x01 => GnssHostStatus::UnexpectedCmd,
            0x02 => GnssHostStatus::UnimplementedCmd,
            0x03 => GnssHostStatus::InvalidParameters,
            0x04 => GnssHostStatus::MessageSanityCheckError,
            0x05 => GnssHostStatus::IqCaptureFails,
            0x06 => GnssHostStatus::NoTime,
            0x07 => GnssHostStatus::NoSatelliteDetected,
            0x08 => GnssHostStatus::AlmanacInFlashTooOld,
            0x09 => GnssHostStatus::AlmanacUpdateFailsCrcError,
            0x0A => GnssHostStatus::AlmanacUpdateFailsFlashIntegrityError,
            0x0C => GnssHostStatus::AlmanacUpdateNotAllowed,
            0x0D => GnssHostStatus::AlmanacCrcError,
            0x0E => GnssHostStatus::AlmanacVersionNotSupported,
            0x10 => GnssHostStatus::NotEnoughSvDetectedToBuildNavMessage,
            0x11 => GnssHostStatus::TimeDemodulationFail,
            0x12 => GnssHostStatus::AlmanacDemodulationFail,
            0x13 => GnssHostStatus::AtLeastTheDetectedSvOfOneConstellationAreDeactivated,
            0x14 => GnssHostStatus::AssistancePositionPossiblyWrongButFailsToUpdate,
            0x15 => GnssHostStatus::ScanAborted,
            0x16 => GnssHostStatus::NavMessageCannotBeGeneratedIntervalGreaterThan63Sec,
            _ => GnssHostStatus::UnexpectedCmd,
        }
    }
}

/// GNSS error codes
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GnssErrorCode {
    NoError = 0,
    AlmanacTooOld = 1,
    UpdateCrcMismatch = 2,
    UpdateFlashMemoryIntegrity = 3,
    /// Impossible to update more than one constellation at a time
    AlmanacUpdateNotAllowed = 4,
}

impl From<u8> for GnssErrorCode {
    fn from(value: u8) -> Self {
        match value {
            0 => GnssErrorCode::NoError,
            1 => GnssErrorCode::AlmanacTooOld,
            2 => GnssErrorCode::UpdateCrcMismatch,
            3 => GnssErrorCode::UpdateFlashMemoryIntegrity,
            4 => GnssErrorCode::AlmanacUpdateNotAllowed,
            _ => GnssErrorCode::NoError,
        }
    }
}

/// GNSS frequency search space
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GnssFreqSearchSpace {
    Hz250 = 0,
    Hz500 = 1,
    Khz1 = 2,
    Khz2 = 3,
}

impl GnssFreqSearchSpace {
    pub fn value(self) -> u8 {
        self as u8
    }
}

impl From<u8> for GnssFreqSearchSpace {
    fn from(value: u8) -> Self {
        match value {
            0 => GnssFreqSearchSpace::Hz250,
            1 => GnssFreqSearchSpace::Hz500,
            2 => GnssFreqSearchSpace::Khz1,
            3 => GnssFreqSearchSpace::Khz2,
            _ => GnssFreqSearchSpace::Hz250,
        }
    }
}

/// Result fields bit mask indicating which information is added in the output payload
#[derive(Clone, Copy)]
pub enum GnssResultFields {
    /// Add Doppler information if set
    DopplerEnable = 0x01,
    /// Add up to 14 Doppler if set - up to 7 if not (valid if DopplerEnable is set)
    DopplerMask = 0x02,
    /// Add bit change if set (SingleScanAnd5FastScans mode only)
    BitChange = 0x04,
    /// Add time demodulation if set (SingleScanAnd5FastScans mode only)
    DemodulateTime = 0x08,
    /// Remove time from NAV if set
    RemoveTimeFromNav = 0x10,
    /// Remove aiding position from NAV if set
    RemoveApFromNav = 0x20,
}

impl GnssResultFields {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Maximal GNSS result buffer size: (128sv * 22bytes + 4bytes for CRC)
pub const GNSS_MAX_RESULT_SIZE: usize = 2820;

/// Size of the almanac of a single satellite when reading
pub const GNSS_SINGLE_ALMANAC_READ_SIZE: usize = 22;

/// Size of the almanac of a single satellite when writing
pub const GNSS_SINGLE_ALMANAC_WRITE_SIZE: usize = 20;

/// Size of the GNSS context status buffer
pub const GNSS_CONTEXT_STATUS_LENGTH: usize = 9;

/// Number of almanacs in full update payload
pub const GNSS_FULL_UPDATE_N_ALMANACS: usize = 128;

/// Assistance position for GNSS
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct GnssAssistancePosition {
    /// Latitude in degrees (-90 to +90)
    pub latitude: f32,
    /// Longitude in degrees (-180 to +180)
    pub longitude: f32,
}

/// GNSS firmware version
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct GnssVersion {
    /// Version of the firmware
    pub gnss_firmware: u8,
    /// Version of the almanac format
    pub gnss_almanac: u8,
}

/// Detected satellite information
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct GnssDetectedSatellite {
    /// Satellite ID
    pub satellite_id: u8,
    /// Carrier-to-noise ratio (C/N) in dB
    pub cnr: i8,
    /// SV doppler in Hz
    pub doppler: i16,
}

/// GNSS context status structure
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct GnssContextStatus {
    /// Firmware version
    pub firmware_version: u8,
    /// Global almanac CRC
    pub global_almanac_crc: u32,
    /// Error code
    pub error_code: GnssErrorCode,
    /// Whether GPS almanac needs update
    pub almanac_update_gps: bool,
    /// Whether BeiDou almanac needs update
    pub almanac_update_beidou: bool,
    /// Frequency search space
    pub freq_search_space: GnssFreqSearchSpace,
}

/// GNSS scan result destination index in result buffer
pub const GNSS_SCAN_RESULT_DESTINATION_INDEX: usize = 0;

/// SNR to CNR offset conversion
pub const GNSS_SNR_TO_CNR_OFFSET: i8 = 31;

/// Scaling factor for latitude conversion (90 degrees)
pub const GNSS_SCALING_LATITUDE: f32 = 90.0;

/// Scaling factor for longitude conversion (180 degrees)
pub const GNSS_SCALING_LONGITUDE: f32 = 180.0;

// =============================================================================
// WiFi Types and Constants (from SWDR001 lr11xx_wifi.c and lr11xx_wifi_types.h)
// =============================================================================

/// WiFi OpCodes (16-bit commands)
#[derive(Clone, Copy, PartialEq)]
pub enum WifiOpCode {
    /// Start WiFi passive scan (0x0300)
    Scan = 0x0300,
    /// Start WiFi passive scan with time limit (0x0301)
    ScanTimeLimit = 0x0301,
    /// Search for country codes (0x0302)
    SearchCountryCode = 0x0302,
    /// Country code with time limit (0x0303)
    CountryCodeTimeLimit = 0x0303,
    /// Get the size of scan results (0x0305)
    GetResultSize = 0x0305,
    /// Read scan results (0x0306)
    ReadResult = 0x0306,
    /// Reset cumulative timing (0x0307)
    ResetCumulTiming = 0x0307,
    /// Read cumulative timing (0x0308)
    ReadCumulTiming = 0x0308,
    /// Get the size of country code results (0x0309)
    GetSizeCountryResult = 0x0309,
    /// Read country codes (0x030A)
    ReadCountryCode = 0x030A,
    /// Configure timestamp for AP phone (0x030B)
    ConfigureTimestampApPhone = 0x030B,
    /// Get WiFi firmware version (0x0320)
    GetVersion = 0x0320,
}

impl WifiOpCode {
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// WiFi channel mask type (bit mask for channels 1-14)
pub type WifiChannelMask = u16;

/// WiFi MAC address length in bytes
pub const WIFI_MAC_ADDRESS_LENGTH: usize = 6;

/// Maximum number of WiFi results
pub const WIFI_MAX_RESULTS: usize = 32;

/// WiFi SSID length in bytes
pub const WIFI_RESULT_SSID_LENGTH: usize = 32;

/// Maximum number of country codes
pub const WIFI_MAX_COUNTRY_CODE: usize = 32;

/// Country code string size
pub const WIFI_STR_COUNTRY_CODE_SIZE: usize = 2;

/// WiFi basic complete result size in bytes
pub const WIFI_BASIC_COMPLETE_RESULT_SIZE: usize = 22;

/// WiFi basic MAC/type/channel result size in bytes
pub const WIFI_BASIC_MAC_TYPE_CHANNEL_RESULT_SIZE: usize = 9;

/// WiFi extended complete result size in bytes
pub const WIFI_EXTENDED_COMPLETE_RESULT_SIZE: usize = 79;

/// Maximum results per chunk read
pub const WIFI_N_RESULTS_MAX_PER_CHUNK: u8 = 32;

/// WiFi cumulative timing size in bytes
pub const WIFI_ALL_CUMULATIVE_TIMING_SIZE: usize = 16;

/// WiFi version size in bytes
pub const WIFI_VERSION_SIZE: usize = 2;

/// WiFi channel mask for channel 1 (2.412 GHz)
pub const WIFI_CHANNEL_1_MASK: WifiChannelMask = 0x0001;
/// WiFi channel mask for channel 2 (2.417 GHz)
pub const WIFI_CHANNEL_2_MASK: WifiChannelMask = 0x0002;
/// WiFi channel mask for channel 3 (2.422 GHz)
pub const WIFI_CHANNEL_3_MASK: WifiChannelMask = 0x0004;
/// WiFi channel mask for channel 4 (2.427 GHz)
pub const WIFI_CHANNEL_4_MASK: WifiChannelMask = 0x0008;
/// WiFi channel mask for channel 5 (2.432 GHz)
pub const WIFI_CHANNEL_5_MASK: WifiChannelMask = 0x0010;
/// WiFi channel mask for channel 6 (2.437 GHz)
pub const WIFI_CHANNEL_6_MASK: WifiChannelMask = 0x0020;
/// WiFi channel mask for channel 7 (2.442 GHz)
pub const WIFI_CHANNEL_7_MASK: WifiChannelMask = 0x0040;
/// WiFi channel mask for channel 8 (2.447 GHz)
pub const WIFI_CHANNEL_8_MASK: WifiChannelMask = 0x0080;
/// WiFi channel mask for channel 9 (2.452 GHz)
pub const WIFI_CHANNEL_9_MASK: WifiChannelMask = 0x0100;
/// WiFi channel mask for channel 10 (2.457 GHz)
pub const WIFI_CHANNEL_10_MASK: WifiChannelMask = 0x0200;
/// WiFi channel mask for channel 11 (2.462 GHz)
pub const WIFI_CHANNEL_11_MASK: WifiChannelMask = 0x0400;
/// WiFi channel mask for channel 12 (2.467 GHz)
pub const WIFI_CHANNEL_12_MASK: WifiChannelMask = 0x0800;
/// WiFi channel mask for channel 13 (2.472 GHz)
pub const WIFI_CHANNEL_13_MASK: WifiChannelMask = 0x1000;
/// WiFi channel mask for channel 14 (2.484 GHz)
pub const WIFI_CHANNEL_14_MASK: WifiChannelMask = 0x2000;
/// WiFi channel mask for all channels (1-14)
pub const WIFI_ALL_CHANNELS_MASK: WifiChannelMask = 0x3FFF;

/// WiFi channel index
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum WifiChannel {
    NoChannel = 0x00,
    Channel1 = 0x01,   // 2.412 GHz
    Channel2 = 0x02,   // 2.417 GHz
    Channel3 = 0x03,   // 2.422 GHz
    Channel4 = 0x04,   // 2.427 GHz
    Channel5 = 0x05,   // 2.432 GHz
    Channel6 = 0x06,   // 2.437 GHz
    Channel7 = 0x07,   // 2.442 GHz
    Channel8 = 0x08,   // 2.447 GHz
    Channel9 = 0x09,   // 2.452 GHz
    Channel10 = 0x0A,  // 2.457 GHz
    Channel11 = 0x0B,  // 2.462 GHz
    Channel12 = 0x0C,  // 2.467 GHz
    Channel13 = 0x0D,  // 2.472 GHz
    Channel14 = 0x0E,  // 2.484 GHz
    AllChannels = 0x0F,
}

impl From<u8> for WifiChannel {
    fn from(value: u8) -> Self {
        match value {
            0x00 => WifiChannel::NoChannel,
            0x01 => WifiChannel::Channel1,
            0x02 => WifiChannel::Channel2,
            0x03 => WifiChannel::Channel3,
            0x04 => WifiChannel::Channel4,
            0x05 => WifiChannel::Channel5,
            0x06 => WifiChannel::Channel6,
            0x07 => WifiChannel::Channel7,
            0x08 => WifiChannel::Channel8,
            0x09 => WifiChannel::Channel9,
            0x0A => WifiChannel::Channel10,
            0x0B => WifiChannel::Channel11,
            0x0C => WifiChannel::Channel12,
            0x0D => WifiChannel::Channel13,
            0x0E => WifiChannel::Channel14,
            0x0F => WifiChannel::AllChannels,
            _ => WifiChannel::NoChannel,
        }
    }
}

/// WiFi signal type for scan configuration
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum WifiSignalTypeScan {
    /// WiFi 802.11b only
    TypeB = 0x01,
    /// WiFi 802.11g only
    TypeG = 0x02,
    /// WiFi 802.11n only (Mixed Mode, not GreenField)
    TypeN = 0x03,
    /// WiFi 802.11b, g, and n
    TypeBGN = 0x04,
}

impl WifiSignalTypeScan {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// WiFi signal type in scan results
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum WifiSignalTypeResult {
    TypeB = 0x01,
    TypeG = 0x02,
    TypeN = 0x03,
}

impl From<u8> for WifiSignalTypeResult {
    fn from(value: u8) -> Self {
        match value {
            0x01 => WifiSignalTypeResult::TypeB,
            0x02 => WifiSignalTypeResult::TypeG,
            0x03 => WifiSignalTypeResult::TypeN,
            _ => WifiSignalTypeResult::TypeB,
        }
    }
}

/// WiFi scan mode
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum WifiScanMode {
    /// Exposes Beacons and Probe Responses until Period Beacon field (Basic result)
    Beacon = 1,
    /// Exposes Management AP frames until Period Beacon field, and other packets until third MAC Address (Basic result)
    BeaconAndPacket = 2,
    /// Exposes Beacons and Probe Responses until FCS field (Extended result). Only WiFi B is scanned.
    FullBeacon = 4,
    /// Exposes Beacons and Probe Responses until end of SSID field (Extended result) - available since firmware 0x0306
    UntilSsid = 5,
}

impl WifiScanMode {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// WiFi result format
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum WifiResultFormat {
    /// Basic complete result (22 bytes)
    BasicComplete,
    /// Basic MAC/type/channel result (9 bytes)
    BasicMacTypeChannel,
    /// Extended full result (79 bytes)
    ExtendedFull,
}

impl WifiResultFormat {
    /// Get the format code for reading results (sent to LR1110)
    pub fn format_code(self) -> u8 {
        match self {
            WifiResultFormat::BasicComplete => 0x01,
            WifiResultFormat::BasicMacTypeChannel => 0x04,
            WifiResultFormat::ExtendedFull => 0x01,
        }
    }

    /// Get the size of a single result in bytes
    pub fn result_size(self) -> usize {
        match self {
            WifiResultFormat::BasicComplete => WIFI_BASIC_COMPLETE_RESULT_SIZE,
            WifiResultFormat::BasicMacTypeChannel => WIFI_BASIC_MAC_TYPE_CHANNEL_RESULT_SIZE,
            WifiResultFormat::ExtendedFull => WIFI_EXTENDED_COMPLETE_RESULT_SIZE,
        }
    }
}

/// WiFi frame type
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum WifiFrameType {
    Management = 0x00,
    Control = 0x01,
    Data = 0x02,
}

impl From<u8> for WifiFrameType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => WifiFrameType::Management,
            0x01 => WifiFrameType::Control,
            0x02 => WifiFrameType::Data,
            _ => WifiFrameType::Management,
        }
    }
}

/// WiFi MAC address origin estimation
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum WifiMacOrigin {
    /// MAC address from a fixed Access Point
    BeaconFixAp = 1,
    /// MAC address from a mobile Access Point
    BeaconMobileAp = 2,
    /// Origin cannot be determined
    Unknown = 3,
}

impl From<u8> for WifiMacOrigin {
    fn from(value: u8) -> Self {
        match value {
            1 => WifiMacOrigin::BeaconFixAp,
            2 => WifiMacOrigin::BeaconMobileAp,
            _ => WifiMacOrigin::Unknown,
        }
    }
}

/// WiFi MAC address type
pub type WifiMacAddress = [u8; WIFI_MAC_ADDRESS_LENGTH];

/// WiFi firmware version
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct WifiVersion {
    pub major: u8,
    pub minor: u8,
}

/// WiFi cumulative timing information
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct WifiCumulativeTimings {
    /// Cumulative time spent during NFE or TOA (microseconds)
    pub rx_detection_us: u32,
    /// Cumulative time spent during preamble detection (microseconds)
    pub rx_correlation_us: u32,
    /// Cumulative time spent during signal acquisition (microseconds)
    pub rx_capture_us: u32,
    /// Cumulative time spent during software demodulation (microseconds)
    pub demodulation_us: u32,
}

/// Basic MAC/type/channel WiFi result (9 bytes)
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct WifiBasicMacTypeChannelResult {
    /// Data rate info byte (contains signal type)
    pub data_rate_info_byte: u8,
    /// Channel info byte (contains channel and RSSI validity)
    pub channel_info_byte: u8,
    /// RSSI in dBm
    pub rssi: i8,
    /// MAC address of the access point
    pub mac_address: WifiMacAddress,
}

impl WifiBasicMacTypeChannelResult {
    /// Extract WiFi signal type from data rate info byte
    pub fn signal_type(&self) -> WifiSignalTypeResult {
        WifiSignalTypeResult::from(self.data_rate_info_byte & 0x03)
    }

    /// Extract channel from channel info byte
    pub fn channel(&self) -> WifiChannel {
        WifiChannel::from(self.channel_info_byte & 0x0F)
    }

    /// Check if RSSI value is valid
    pub fn rssi_valid(&self) -> bool {
        (self.channel_info_byte & 0x40) == 0
    }

    /// Extract MAC origin estimation from channel info byte
    pub fn mac_origin(&self) -> WifiMacOrigin {
        WifiMacOrigin::from((self.channel_info_byte & 0x30) >> 4)
    }
}

/// Basic complete WiFi result (22 bytes)
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct WifiBasicCompleteResult {
    /// Data rate info byte
    pub data_rate_info_byte: u8,
    /// Channel info byte
    pub channel_info_byte: u8,
    /// RSSI in dBm
    pub rssi: i8,
    /// Frame type info byte
    pub frame_type_info_byte: u8,
    /// MAC address of the access point
    pub mac_address: WifiMacAddress,
    /// Phase offset
    pub phi_offset: i16,
    /// Timestamp indicating the up-time of the AP transmitting the Beacon (microseconds)
    pub timestamp_us: u64,
    /// Beacon period in TU (1 TU = 1024 microseconds)
    pub beacon_period_tu: u16,
}

impl WifiBasicCompleteResult {
    /// Extract WiFi signal type from data rate info byte
    pub fn signal_type(&self) -> WifiSignalTypeResult {
        WifiSignalTypeResult::from(self.data_rate_info_byte & 0x03)
    }

    /// Extract channel from channel info byte
    pub fn channel(&self) -> WifiChannel {
        WifiChannel::from(self.channel_info_byte & 0x0F)
    }

    /// Check if RSSI value is valid
    pub fn rssi_valid(&self) -> bool {
        (self.channel_info_byte & 0x40) == 0
    }

    /// Extract MAC origin estimation
    pub fn mac_origin(&self) -> WifiMacOrigin {
        WifiMacOrigin::from((self.channel_info_byte & 0x30) >> 4)
    }

    /// Extract frame type from frame type info byte
    pub fn frame_type(&self) -> WifiFrameType {
        WifiFrameType::from((self.frame_type_info_byte >> 6) & 0x03)
    }
}

/// FCS (Frame Check Sequence) info
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct WifiFcsInfo {
    /// True if FCS was checked
    pub is_fcs_checked: bool,
    /// True if FCS check passed
    pub is_fcs_ok: bool,
}

/// Extended full WiFi result (79 bytes)
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct WifiExtendedFullResult {
    /// Data rate info byte
    pub data_rate_info_byte: u8,
    /// Channel info byte
    pub channel_info_byte: u8,
    /// RSSI in dBm
    pub rssi: i8,
    /// Rate index
    pub rate: u8,
    /// Service value
    pub service: u16,
    /// Length of MPDU (microseconds for WiFi B, bytes for WiFi G)
    pub length: u16,
    /// Frame control structure
    pub frame_control: u16,
    /// MAC address 1
    pub mac_address_1: WifiMacAddress,
    /// MAC address 2
    pub mac_address_2: WifiMacAddress,
    /// MAC address 3
    pub mac_address_3: WifiMacAddress,
    /// Timestamp indicating the up-time of the AP (microseconds)
    pub timestamp_us: u64,
    /// Beacon period in TU
    pub beacon_period_tu: u16,
    /// Sequence control value
    pub seq_control: u16,
    /// SSID bytes (Service Set IDentifier)
    pub ssid_bytes: [u8; WIFI_RESULT_SSID_LENGTH],
    /// Current channel indicated in the WiFi frame
    pub current_channel: WifiChannel,
    /// Country code (2 characters)
    pub country_code: [u8; WIFI_STR_COUNTRY_CODE_SIZE],
    /// Input/Output regulation
    pub io_regulation: u8,
    /// FCS check info
    pub fcs_check_byte: WifiFcsInfo,
    /// Phase offset
    pub phi_offset: i16,
}

impl Default for WifiExtendedFullResult {
    fn default() -> Self {
        Self {
            data_rate_info_byte: 0,
            channel_info_byte: 0,
            rssi: 0,
            rate: 0,
            service: 0,
            length: 0,
            frame_control: 0,
            mac_address_1: [0u8; WIFI_MAC_ADDRESS_LENGTH],
            mac_address_2: [0u8; WIFI_MAC_ADDRESS_LENGTH],
            mac_address_3: [0u8; WIFI_MAC_ADDRESS_LENGTH],
            timestamp_us: 0,
            beacon_period_tu: 0,
            seq_control: 0,
            ssid_bytes: [0u8; WIFI_RESULT_SSID_LENGTH],
            current_channel: WifiChannel::NoChannel,
            country_code: [0u8; WIFI_STR_COUNTRY_CODE_SIZE],
            io_regulation: 0,
            fcs_check_byte: WifiFcsInfo::default(),
            phi_offset: 0,
        }
    }
}

impl WifiExtendedFullResult {
    /// Extract WiFi signal type from data rate info byte
    pub fn signal_type(&self) -> WifiSignalTypeResult {
        WifiSignalTypeResult::from(self.data_rate_info_byte & 0x03)
    }

    /// Extract channel from channel info byte
    pub fn channel(&self) -> WifiChannel {
        WifiChannel::from(self.channel_info_byte & 0x0F)
    }

    /// Get SSID as string (if valid UTF-8)
    pub fn ssid_str(&self) -> Option<&str> {
        // Find null terminator
        let len = self.ssid_bytes.iter().position(|&c| c == 0).unwrap_or(WIFI_RESULT_SSID_LENGTH);
        core::str::from_utf8(&self.ssid_bytes[..len]).ok()
    }
}

// =============================================================================
// Crypto Engine Types and Constants (from SWDR001 lr11xx_crypto_engine.c/h)
// =============================================================================

/// Crypto Engine OpCodes (16-bit commands)
#[derive(Clone, Copy, PartialEq)]
pub enum CryptoOpCode {
    /// Select crypto element (internal or secure element) (0x0500)
    Select = 0x0500,
    /// Set a key in the crypto engine (0x0502)
    SetKey = 0x0502,
    /// Derive a key from another key (0x0503)
    DeriveKey = 0x0503,
    /// Process LoRaWAN Join Accept message (0x0504)
    ProcessJoinAccept = 0x0504,
    /// Compute AES-CMAC (0x0505)
    ComputeAesCmac = 0x0505,
    /// Verify AES-CMAC (0x0506)
    VerifyAesCmac = 0x0506,
    /// AES encrypt (legacy, variant 01) (0x0507)
    AesEncrypt01 = 0x0507,
    /// AES encrypt (0x0508)
    AesEncrypt = 0x0508,
    /// AES decrypt (0x0509)
    AesDecrypt = 0x0509,
    /// Store crypto data to flash (0x050A)
    StoreToFlash = 0x050A,
    /// Restore crypto data from flash (0x050B)
    RestoreFromFlash = 0x050B,
    /// Set a crypto parameter (0x050D)
    SetParameter = 0x050D,
    /// Get a crypto parameter (0x050E)
    GetParameter = 0x050E,
    /// Check encrypted firmware image (0x050F)
    CheckEncryptedFwImage = 0x050F,
    /// Get result of encrypted firmware image check (0x0510)
    GetCheckEncryptedFwImageResult = 0x0510,
}

impl CryptoOpCode {
    /// Convert opcode to bytes for SPI command
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// Length of MIC (Message Integrity Code) in bytes
pub const CRYPTO_MIC_LENGTH: usize = 4;

/// Length of AES-CMAC in bytes
pub const CRYPTO_AES_CMAC_LENGTH: usize = 16;

/// Maximum length of data to encrypt/decrypt in bytes
pub const CRYPTO_DATA_MAX_LENGTH: usize = 256;

/// Length of AES key in bytes
pub const CRYPTO_KEY_LENGTH: usize = 16;

/// Length of nonce in bytes
pub const CRYPTO_NONCE_LENGTH: usize = 16;

/// Length of crypto parameter in bytes
pub const CRYPTO_PARAMETER_LENGTH: usize = 4;

/// Crypto element selection
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum CryptoElement {
    /// Internal crypto engine (default)
    CryptoEngine = 0x00,
    /// External secure element
    SecureElement = 0x01,
}

impl CryptoElement {
    /// Get the value for SPI command
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Status returned by crypto operations
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum CryptoStatus {
    /// Operation successful
    Success = 0x00,
    /// AES-CMAC invalid or comparison failed
    ErrorFailCmac = 0x01,
    /// Invalid key ID (source or destination)
    ErrorInvalidKeyId = 0x03,
    /// Invalid data buffer size
    ErrorBufferSize = 0x05,
    /// Other error
    Error = 0x06,
}

impl From<u8> for CryptoStatus {
    fn from(value: u8) -> Self {
        match value {
            0x00 => CryptoStatus::Success,
            0x01 => CryptoStatus::ErrorFailCmac,
            0x03 => CryptoStatus::ErrorInvalidKeyId,
            0x05 => CryptoStatus::ErrorBufferSize,
            _ => CryptoStatus::Error,
        }
    }
}

/// LoRaWAN version for crypto operations
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum CryptoLorawanVersion {
    /// LoRaWAN 1.0.x
    V1_0x = 0x00,
    /// LoRaWAN 1.1.x
    V1_1x = 0x01,
}

impl CryptoLorawanVersion {
    /// Get the value for SPI command
    pub fn value(self) -> u8 {
        self as u8
    }

    /// Get header length for this LoRaWAN version
    pub fn header_length(self) -> usize {
        match self {
            CryptoLorawanVersion::V1_0x => 1,
            CryptoLorawanVersion::V1_1x => 12,
        }
    }
}

/// Crypto key slot identifiers
///
/// The LR1110 has dedicated key slots for LoRaWAN operations
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[repr(u8)]
pub enum CryptoKeyId {
    /// Mother key (root key for derivation)
    MotherKey = 1,
    /// Network key (NwkKey)
    NwkKey = 2,
    /// Application key (AppKey)
    AppKey = 3,
    /// Join server encryption key (JSEncKey)
    JSEncKey = 4,
    /// Join server integrity key (JSIntKey)
    JSIntKey = 5,
    /// General purpose key encryption key 0
    GpKeKey0 = 6,
    /// General purpose key encryption key 1
    GpKeKey1 = 7,
    /// General purpose key encryption key 2
    GpKeKey2 = 8,
    /// General purpose key encryption key 3
    GpKeKey3 = 9,
    /// General purpose key encryption key 4
    GpKeKey4 = 10,
    /// General purpose key encryption key 5
    GpKeKey5 = 11,
    /// Application session key (AppSKey)
    AppSKey = 12,
    /// Forwarding network session integrity key (FNwkSIntKey)
    FNwkSIntKey = 13,
    /// Serving network session integrity key (SNwkSIntKey)
    SNwkSIntKey = 14,
    /// Network session encryption key (NwkSEncKey)
    NwkSEncKey = 15,
    /// Reserved 0
    Rfu0 = 16,
    /// Reserved 1
    Rfu1 = 17,
    /// Multicast application session key 0
    McAppSKey0 = 18,
    /// Multicast application session key 1
    McAppSKey1 = 19,
    /// Multicast application session key 2
    McAppSKey2 = 20,
    /// Multicast application session key 3
    McAppSKey3 = 21,
    /// Multicast network session key 0
    McNwkSKey0 = 22,
    /// Multicast network session key 1
    McNwkSKey1 = 23,
    /// Multicast network session key 2
    McNwkSKey2 = 24,
    /// Multicast network session key 3
    McNwkSKey3 = 25,
    /// General purpose key 0
    Gp0 = 26,
    /// General purpose key 1
    Gp1 = 27,
}

impl CryptoKeyId {
    /// Get the key ID value
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Type alias for crypto key (16 bytes)
pub type CryptoKey = [u8; CRYPTO_KEY_LENGTH];

/// Type alias for crypto nonce (16 bytes)
pub type CryptoNonce = [u8; CRYPTO_NONCE_LENGTH];

/// Type alias for MIC (4 bytes)
pub type CryptoMic = [u8; CRYPTO_MIC_LENGTH];

/// Type alias for crypto parameter (4 bytes)
pub type CryptoParam = [u8; CRYPTO_PARAMETER_LENGTH];

// =============================================================================
// RTToF (Round-Trip Time of Flight) Types and Constants (from SWDR001 lr11xx_rttof.c/h)
// =============================================================================

/// RTToF OpCodes (16-bit commands)
///
/// Note: RTToF opcodes are in the 0x02XX range (shared with Radio opcodes)
#[derive(Clone, Copy, PartialEq)]
pub enum RttofOpCode {
    /// Set the subordinate device address (0x021C)
    SetAddress = 0x021C,
    /// Set the request address for manager mode (0x021D)
    SetRequestAddress = 0x021D,
    /// Get RTToF result (0x021E)
    GetResult = 0x021E,
    /// Set RX/TX delay indicator for calibration (0x021F)
    SetRxTxDelay = 0x021F,
    /// Set RTToF parameters (0x0228)
    SetParameters = 0x0228,
}

impl RttofOpCode {
    /// Convert opcode to bytes for SPI command
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// Length of RTToF result in bytes
pub const RTTOF_RESULT_LENGTH: usize = 4;

/// Default RTToF address
pub const RTTOF_DEFAULT_ADDRESS: u32 = 0x00000019;

/// Default number of symbols for RTToF (recommended value)
pub const RTTOF_DEFAULT_NB_SYMBOLS: u8 = 15;

/// RTToF result type
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum RttofResultType {
    /// Raw distance result (needs conversion to meters)
    Raw = 0x00,
    /// RSSI result
    Rssi = 0x01,
}

impl RttofResultType {
    /// Get the value for SPI command
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Type alias for RTToF raw result (4 bytes)
pub type RttofRawResult = [u8; RTTOF_RESULT_LENGTH];

/// RTToF distance result with metadata
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct RttofDistanceResult {
    /// Distance in meters (can be negative for calibration offsets)
    pub distance_m: i32,
    /// RSSI in dBm
    pub rssi_dbm: i8,
}

/// Convert raw RTToF distance result to meters
///
/// # Arguments
/// * `bandwidth` - LoRa bandwidth used during RTToF measurement
/// * `raw_result` - 4-byte raw distance result from device
///
/// # Returns
/// Distance in meters (can be negative)
pub fn rttof_distance_raw_to_meters(bandwidth: Bandwidth, raw_result: &RttofRawResult) -> i32 {
    let bitcnt: u8 = 24;

    // Bandwidth scaling factor
    let bw_scaling: i32 = match bandwidth {
        Bandwidth::_500KHz => 1,
        Bandwidth::_250KHz => 2,
        Bandwidth::_125KHz => 4,
        _ => 1, // Default to 500 kHz scaling for unsupported bandwidths
    };

    // Parse raw distance (big-endian)
    let raw_distance: u32 = ((raw_result[0] as u32) << 24)
        | ((raw_result[1] as u32) << 16)
        | ((raw_result[2] as u32) << 8)
        | (raw_result[3] as u32);

    // Convert to signed value (24-bit two's complement)
    let mut retval = raw_distance as i32;
    if raw_distance >= (1u32 << (bitcnt - 1)) {
        retval -= 1i32 << bitcnt;
    }

    // Calculate distance: 300 * bw_scaling * raw / 4096
    300 * bw_scaling * retval / 4096
}

/// Convert raw RTToF RSSI result to dBm
///
/// # Arguments
/// * `raw_result` - 4-byte raw RSSI result from device
///
/// # Returns
/// RSSI in dBm
pub fn rttof_rssi_raw_to_dbm(raw_result: &RttofRawResult) -> i8 {
    // Only the last byte is meaningful
    -((raw_result[3] >> 1) as i8)
}

// =============================================================================
// RTToF Ranging Constants for Demo Application
// =============================================================================

/// Packet type values for use with set_packet_type()
pub mod packet_type {
    /// LoRa packet type
    pub const LORA: u8 = 0x02;
    /// RTToF (Round-Trip Time of Flight) packet type for ranging
    pub const RTTOF: u8 = 0x05;
}

/// IRQ masks for RTToF ranging
pub mod ranging_irq {
    use super::IrqMask;

    /// LoRa IRQ mask for initialization phase (RxDone, TxDone, Timeout, CrcError)
    pub const LORA_IRQ_MASK: u32 =
        IrqMask::TxDone.value() | IrqMask::RxDone.value() |
        IrqMask::Timeout.value() | IrqMask::CrcError.value() |
        IrqMask::HeaderError.value();

    /// Manager device IRQ mask for RTToF ranging
    /// RttofExchValid = ranging exchange completed successfully
    /// RttofTimeout = ranging timeout
    pub const MANAGER_IRQ_MASK: u32 =
        IrqMask::RttofExchValid.value() | IrqMask::RttofTimeout.value();

    /// Subordinate device IRQ mask for RTToF ranging
    /// RttofReqValid = received valid ranging request
    /// RttofRespDone = sent ranging response
    /// RttofReqDiscarded = discarded ranging request (address mismatch)
    pub const SUBORDINATE_IRQ_MASK: u32 =
        IrqMask::RttofReqValid.value() | IrqMask::RttofRespDone.value() |
        IrqMask::RttofReqDiscarded.value();
}

/// Ranging configuration constants (matching lr11xx_ranging_demo)
pub mod ranging_config {
    /// Default ranging address
    pub const DEFAULT_ADDRESS: u32 = 0x32101222;

    /// Number of address bytes the subordinate checks (1-4)
    pub const SUBORDINATE_CHECK_LENGTH_BYTES: u8 = 4;

    /// Number of symbols in ranging response
    pub const RESPONSE_SYMBOLS_COUNT: u8 = 15;

    /// Payload length for LoRa initialization packets
    pub const INIT_PAYLOAD_LENGTH: usize = 6;

    /// Processing time between ranging channels (ms)
    pub const DONE_PROCESSING_TIME_MS: u32 = 5;

    /// Maximum number of frequency hopping channels
    pub const MAX_HOPPING_CHANNELS: usize = 39;

    /// Minimum successful measurements for valid result
    pub const MIN_HOPPING_CHANNELS: usize = 10;

    /// LoRa sync word for private network
    pub const LORA_SYNC_WORD: u8 = 0x34;

    /// Continuous RX timeout value
    pub const RX_CONTINUOUS: u32 = 0xFFFFFF;
}

/// Frequency hopping channel tables for different regions
pub mod ranging_channels {
    /// ISM 902-928 MHz (US915) - 39 channels
    pub const US915: [u32; 39] = [
        907850000, 902650000, 914350000, 906550000, 905900000, 924750000, 926700000, 918250000, 921500000, 909150000,
        907200000, 924100000, 903950000, 910450000, 917600000, 919550000, 923450000, 925400000, 909800000, 915000000,
        912400000, 904600000, 908500000, 911100000, 911750000, 916300000, 918900000, 905250000, 913700000, 927350000,
        926050000, 916950000, 913050000, 903300000, 920200000, 922800000, 915650000, 922150000, 920850000,
    ];

    /// ISM 863-870 MHz (EU868) - 39 channels
    pub const EU868: [u32; 39] = [
        863750000, 865100000, 864800000, 868400000, 865250000, 867500000, 865550000, 867650000, 866150000, 864050000,
        867800000, 863300000, 863450000, 867950000, 868550000, 868850000, 867200000, 867050000, 864650000, 863900000,
        864500000, 866450000, 865400000, 868700000, 863150000, 866750000, 866300000, 864950000, 864350000, 866000000,
        866900000, 868250000, 865850000, 865700000, 867350000, 868100000, 863600000, 866600000, 864200000,
    ];

    /// ISM 490-510 MHz (CN490) - 39 channels
    pub const CN490: [u32; 39] = [
        490810000, 508940000, 496690000, 507470000, 504040000, 508450000, 505020000, 497670000, 497180000, 500610000,
        494240000, 493260000, 495710000, 491300000, 504530000, 501100000, 502080000, 501590000, 499140000, 494730000,
        506980000, 492280000, 509430000, 495220000, 492770000, 507960000, 493750000, 499630000, 496200000, 498160000,
        505510000, 500120000, 503060000, 506000000, 506490000, 498650000, 491790000, 503550000, 502570000,
    ];

    /// ISM 2.4 GHz - 39 channels
    pub const ISM2G4: [u32; 39] = [
        2450000000, 2402000000, 2476000000, 2436000000, 2430000000, 2468000000, 2458000000, 2416000000,
        2424000000, 2478000000, 2456000000, 2448000000, 2462000000, 2472000000, 2432000000, 2446000000,
        2422000000, 2442000000, 2460000000, 2474000000, 2414000000, 2464000000, 2454000000, 2444000000,
        2404000000, 2434000000, 2410000000, 2408000000, 2440000000, 2452000000, 2480000000, 2426000000,
        2428000000, 2466000000, 2418000000, 2412000000, 2406000000, 2470000000, 2438000000,
    ];
}

/// LoRa spreading factor values
pub mod lora_sf {
    /// SF5
    pub const SF5: u8 = 0x05;
    /// SF6
    pub const SF6: u8 = 0x06;
    /// SF7
    pub const SF7: u8 = 0x07;
    /// SF8
    pub const SF8: u8 = 0x08;
    /// SF9
    pub const SF9: u8 = 0x09;
    /// SF10
    pub const SF10: u8 = 0x0A;
    /// SF11
    pub const SF11: u8 = 0x0B;
    /// SF12
    pub const SF12: u8 = 0x0C;
}

/// LoRa bandwidth values
pub mod lora_bw {
    /// 125 kHz
    pub const BW_125: u8 = 0x04;
    /// 250 kHz
    pub const BW_250: u8 = 0x05;
    /// 500 kHz
    pub const BW_500: u8 = 0x06;
}

/// LoRa coding rate values
pub mod lora_cr {
    /// CR 4/5
    pub const CR_4_5: u8 = 0x01;
    /// CR 4/6
    pub const CR_4_6: u8 = 0x02;
    /// CR 4/7
    pub const CR_4_7: u8 = 0x03;
    /// CR 4/8
    pub const CR_4_8: u8 = 0x04;
}

/// Calculate single symbol time in milliseconds
///
/// # Arguments
/// * `bw` - Bandwidth value (from lora_bw module)
/// * `sf` - Spreading factor value (from lora_sf module)
///
/// # Returns
/// Symbol time in milliseconds as f32
pub fn calculate_symbol_time_ms(bw: u8, sf: u8) -> f32 {
    let bw_khz: f32 = match bw {
        0x04 => 125.0,  // BW_125
        0x05 => 250.0,  // BW_250
        0x06 => 500.0,  // BW_500
        _ => 500.0,
    };

    let sf_val = sf as u32;
    let symbol_time_ms = (1u32 << sf_val) as f32 / bw_khz;
    symbol_time_ms
}

/// Calculate ranging request delay in milliseconds
///
/// This calculates the time for a complete ranging exchange including:
/// - Preamble
/// - Frequency sync (4.25 symbols, 6.25 for SF5/SF6)
/// - Double header (16 symbols)
/// - Ranging request (15 symbols)
/// - Ranging silence (2 symbols)
/// - Response symbols
///
/// # Arguments
/// * `bw` - Bandwidth value
/// * `sf` - Spreading factor value
/// * `preamble_len` - Preamble length in symbols
/// * `response_symbols` - Number of response symbols
///
/// # Returns
/// Delay in milliseconds
pub fn calculate_ranging_request_delay_ms(bw: u8, sf: u8, preamble_len: u16, response_symbols: u8) -> u32 {
    let symbol_time_ms = calculate_symbol_time_ms(bw, sf);

    // Extra symbols for SF5/SF6
    let extra_symbols: f32 = if sf == lora_sf::SF5 || sf == lora_sf::SF6 {
        2.0
    } else {
        0.0
    };

    // Total symbols for ranging exchange
    // Preamble + FreqSync(4.25) + DoubleHeader(16) + Request(15) + Silence(2) + Response
    let freq_sync_symbols = 4.25;
    let double_header_symbols = 16.0;
    let request_symbols = 15.0;
    let silence_symbols = 2.0;

    let total_symbols = preamble_len as f32
        + freq_sync_symbols
        + double_header_symbols
        + request_symbols
        + silence_symbols
        + response_symbols as f32
        + extra_symbols;

    // Add PA ramp time (approximately 0.3ms for typical values) and processing time
    let pa_ramp_ms = 0.3;
    let delay_ms = (symbol_time_ms * total_symbols) + pa_ramp_ms + ranging_config::DONE_PROCESSING_TIME_MS as f32 + 1.0;

    delay_ms as u32
}

// =============================================================================
// Bootloader Types and Constants (from SWDR001 lr11xx_bootloader.c/h)
// =============================================================================

/// Bootloader OpCodes (16-bit commands)
///
/// These opcodes are used when the chip is in bootloader mode (before flash
/// code execution or during firmware update).
#[derive(Clone, Copy, PartialEq)]
pub enum BootloaderOpCode {
    /// Get status registers (0x0100) - same as System GetStatus
    GetStatus = 0x0100,
    /// Get bootloader version (0x0101) - same as System GetVersion
    GetVersion = 0x0101,
    /// Erase entire flash memory (0x8000)
    EraseFlash = 0x8000,
    /// Write encrypted data to flash (0x8003)
    WriteFlashEncrypted = 0x8003,
    /// Reboot the chip (0x8005)
    Reboot = 0x8005,
    /// Read device PIN for cloud claiming (0x800B)
    GetPin = 0x800B,
    /// Read chip EUI (0x800C)
    ReadChipEui = 0x800C,
    /// Read join EUI (0x800D)
    ReadJoinEui = 0x800D,
}

impl BootloaderOpCode {
    /// Convert opcode to bytes for SPI command
    pub fn bytes(self) -> [u8; 2] {
        let val = self as u16;
        [(val >> 8) as u8, (val & 0xFF) as u8]
    }
}

/// Length of bootloader version in bytes
pub const BOOTLOADER_VERSION_LENGTH: usize = 4;

/// Length of PIN in bytes
pub const BOOTLOADER_PIN_LENGTH: usize = 4;

/// Length of chip EUI in bytes
pub const BOOTLOADER_CHIP_EUI_LENGTH: usize = 8;

/// Length of join EUI in bytes
pub const BOOTLOADER_JOIN_EUI_LENGTH: usize = 8;

/// Maximum flash write block size in 32-bit words
pub const BOOTLOADER_FLASH_BLOCK_SIZE_WORDS: usize = 64;

/// Maximum flash write block size in bytes
pub const BOOTLOADER_FLASH_BLOCK_SIZE_BYTES: usize = BOOTLOADER_FLASH_BLOCK_SIZE_WORDS * 4;

/// Type alias for bootloader PIN (4 bytes)
pub type BootloaderPin = [u8; BOOTLOADER_PIN_LENGTH];

/// Type alias for chip EUI (8 bytes)
pub type BootloaderChipEui = [u8; BOOTLOADER_CHIP_EUI_LENGTH];

/// Type alias for join EUI (8 bytes)
pub type BootloaderJoinEui = [u8; BOOTLOADER_JOIN_EUI_LENGTH];

/// Bootloader version information
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct BootloaderVersion {
    /// Hardware version
    pub hw: u8,
    /// Chip type (same encoding as system Version)
    pub chip_type: u8,
    /// Firmware version (bootloader version)
    pub fw: u16,
}

/// Bootloader command status
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum BootloaderCommandStatus {
    /// Command failed
    Fail = 0x00,
    /// Permission error
    Perr = 0x01,
    /// Command OK
    Ok = 0x02,
    /// Data available
    Data = 0x03,
}

impl From<u8> for BootloaderCommandStatus {
    fn from(value: u8) -> Self {
        match value & 0x07 {
            0x00 => BootloaderCommandStatus::Fail,
            0x01 => BootloaderCommandStatus::Perr,
            0x02 => BootloaderCommandStatus::Ok,
            0x03 => BootloaderCommandStatus::Data,
            _ => BootloaderCommandStatus::Fail,
        }
    }
}

/// Bootloader status register 1
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct BootloaderStat1 {
    /// Command status
    pub command_status: u8,
    /// Interrupt is active
    pub is_interrupt_active: bool,
}

impl BootloaderStat1 {
    /// Parse from raw byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            is_interrupt_active: (byte & 0x01) != 0,
            command_status: byte >> 1,
        }
    }

    /// Get command status as enum
    pub fn status(&self) -> BootloaderCommandStatus {
        BootloaderCommandStatus::from(self.command_status)
    }
}

/// Bootloader status register 2
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct BootloaderStat2 {
    /// Chip is running from flash (vs bootloader)
    pub is_running_from_flash: bool,
    /// Current chip mode
    pub chip_mode: u8,
    /// Reset status
    pub reset_status: u8,
}

impl BootloaderStat2 {
    /// Parse from raw byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            is_running_from_flash: (byte & 0x01) != 0,
            chip_mode: (byte & 0x0F) >> 1,
            reset_status: (byte & 0xF0) >> 4,
        }
    }
}

/// Complete bootloader status
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct BootloaderStatus {
    /// Status register 1
    pub stat1: BootloaderStat1,
    /// Status register 2
    pub stat2: BootloaderStat2,
    /// IRQ status flags (32-bit mask)
    pub irq_status: u32,
}

// =============================================================================
// Radio Timings (from SWDR001 lr11xx_radio_timings.h/.c)
// =============================================================================

/// Time in microseconds taken by the chip to process the Rx done interrupt
pub const RX_DONE_IRQ_PROCESSING_TIME_IN_US: u32 = 74;

/// Time in microseconds taken by the chip to process the Tx done interrupt
pub const TX_DONE_IRQ_PROCESSING_TIME_IN_US: u32 = 111;

impl RampTime {
    /// Convert PA ramp time to microseconds
    pub const fn to_us(self) -> u32 {
        match self {
            RampTime::Ramp16Us => 16,
            RampTime::Ramp32Us => 32,
            RampTime::Ramp48Us => 48,
            RampTime::Ramp64Us => 64,
            RampTime::Ramp80Us => 80,
            RampTime::Ramp96Us => 96,
            RampTime::Ramp112Us => 112,
            RampTime::Ramp128Us => 128,
            RampTime::Ramp144Us => 144,
            RampTime::Ramp160Us => 160,
            RampTime::Ramp176Us => 176,
            RampTime::Ramp192Us => 192,
            RampTime::Ramp208Us => 208,
            RampTime::Ramp240Us => 240,
            RampTime::Ramp272Us => 272,
            RampTime::Ramp304Us => 304,
        }
    }
}

/// Get the LoRa reception input delay for a given bandwidth
///
/// This delay depends on the radio's digital filter settling time.
/// Values are from SWDR001 for the common LoRaWAN bandwidths.
pub fn lora_rx_input_delay_in_us(bandwidth: Bandwidth) -> u32 {
    match bandwidth {
        Bandwidth::_500KHz => 16,
        Bandwidth::_250KHz => 31,
        Bandwidth::_125KHz => 57,
        // Lower bandwidths have longer settling times (extrapolated)
        Bandwidth::_62KHz => 114,
        Bandwidth::_41KHz => 171,
        Bandwidth::_31KHz => 228,
        Bandwidth::_20KHz => 342,
        Bandwidth::_15KHz => 456,
        Bandwidth::_10KHz => 684,
        Bandwidth::_7KHz => 912,
    }
}

/// Get the LoRa symbol time for a given spreading factor and bandwidth
///
/// Symbol time = 2^SF / BW (in seconds)
/// Returns time in microseconds
pub fn lora_symbol_time_in_us(spreading_factor: SpreadingFactor, bandwidth: Bandwidth) -> u32 {
    let sf = spreading_factor.factor();
    let bw_hz = bandwidth.hz();
    // (2^SF * 1_000_000) / BW_Hz
    (1u32 << sf) * 1_000_000 / bw_hz
}

/// Get the time between the last bit sent (on Tx side) and the Rx done event (on Rx side)
///
/// This includes:
/// - RX input delay (filter settling time)
/// - 2 symbol times (for synchronization)
/// - RX done IRQ processing time
///
/// This timing is useful for LoRaWAN RX window calculations.
pub fn delay_between_last_bit_sent_and_rx_done_in_us(
    spreading_factor: SpreadingFactor,
    bandwidth: Bandwidth,
) -> u32 {
    lora_rx_input_delay_in_us(bandwidth)
        + 2 * lora_symbol_time_in_us(spreading_factor, bandwidth)
        + RX_DONE_IRQ_PROCESSING_TIME_IN_US
}

/// Get the time between the last bit sent and the Tx done event
///
/// This includes:
/// - PA ramp down time (same as ramp up)
/// - TX done IRQ processing time
///
/// This timing is useful for precise transmit timing calculations.
pub fn delay_between_last_bit_sent_and_tx_done_in_us(ramp_time: RampTime) -> u32 {
    ramp_time.to_us() + TX_DONE_IRQ_PROCESSING_TIME_IN_US
}
