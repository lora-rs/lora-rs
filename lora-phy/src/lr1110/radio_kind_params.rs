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
    TxDone = 0x00000004,              // bit 2
    RxDone = 0x00000008,              // bit 3
    PreambleDetected = 0x00000010,    // bit 4
    SyncWordHeaderValid = 0x00000020, // bit 5
    HeaderError = 0x00000040,         // bit 6
    CrcError = 0x00000080,            // bit 7
    CadDone = 0x00000100,             // bit 8
    CadDetected = 0x00000200,         // bit 9
    Timeout = 0x00000400,             // bit 10
    LrFhssIntraPktHop = 0x00000800,   // bit 11
    RttofReqValid = 0x00004000,       // bit 14
    RttofReqDiscarded = 0x00008000,   // bit 15
    RttofRespDone = 0x00010000,       // bit 16
    RttofExchValid = 0x00020000,      // bit 17
    RttofTimeout = 0x00040000,        // bit 18
    GnssScanDone = 0x00080000,        // bit 19
    WifiScanDone = 0x00100000,        // bit 20
    Eol = 0x00200000,                 // bit 21
    CmdError = 0x00400000,            // bit 22
    Error = 0x00800000,               // bit 23
    FskLenError = 0x01000000,         // bit 24
    FskAddrError = 0x02000000,        // bit 25
    LoRaRxTimestamp = 0x08000000,     // bit 27
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
    Lp = 0x00, // Low-power PA (up to +14dBm)
    Hp = 0x01, // High-power PA (up to +22dBm)
    Hf = 0x02, // High-frequency PA (2.4GHz)
}

impl PaSelection {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Power Amplifier regulator supply
#[derive(Clone, Copy)]
pub enum PaRegSupply {
    Vreg = 0x00, // From internal regulator
    Vbat = 0x01, // From battery
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
        Bandwidth::_7KHz => Err(RadioError::InvalidBandwidthForFrequency), // Not supported on LR1110
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

// =============================================================================
// GFSK Types and Constants (from SWDR001 lr11xx_radio.c/h)
// =============================================================================

/// GFSK pulse shaping filter
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GfskPulseShape {
    /// No filter
    Off = 0x00,
    /// Gaussian BT=0.3
    Bt03 = 0x08,
    /// Gaussian BT=0.5
    Bt05 = 0x09,
    /// Gaussian BT=0.7
    Bt07 = 0x0A,
    /// Gaussian BT=1
    Bt1 = 0x0B,
}

impl GfskPulseShape {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// GFSK bandwidth (receiver bandwidth for RX)
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GfskBandwidth {
    /// 4.8 kHz DSB
    Bw4800 = 0x1F,
    /// 5.8 kHz DSB
    Bw5800 = 0x17,
    /// 7.3 kHz DSB
    Bw7300 = 0x0F,
    /// 9.7 kHz DSB
    Bw9700 = 0x1E,
    /// 11.7 kHz DSB
    Bw11700 = 0x16,
    /// 14.6 kHz DSB
    Bw14600 = 0x0E,
    /// 19.5 kHz DSB
    Bw19500 = 0x1D,
    /// 23.4 kHz DSB
    Bw23400 = 0x15,
    /// 29.3 kHz DSB
    Bw29300 = 0x0D,
    /// 39.0 kHz DSB
    Bw39000 = 0x1C,
    /// 46.9 kHz DSB
    Bw46900 = 0x14,
    /// 58.6 kHz DSB
    Bw58600 = 0x0C,
    /// 78.2 kHz DSB
    Bw78200 = 0x1B,
    /// 93.8 kHz DSB
    Bw93800 = 0x13,
    /// 117.3 kHz DSB
    Bw117300 = 0x0B,
    /// 156.2 kHz DSB
    Bw156200 = 0x1A,
    /// 187.2 kHz DSB
    Bw187200 = 0x12,
    /// 234.3 kHz DSB
    Bw234300 = 0x0A,
    /// 312.0 kHz DSB
    Bw312000 = 0x19,
    /// 373.6 kHz DSB
    Bw373600 = 0x11,
    /// 467.0 kHz DSB
    Bw467000 = 0x09,
}

impl GfskBandwidth {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// GFSK preamble detector length
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GfskPreambleDetector {
    /// Preamble detection disabled
    Off = 0x00,
    /// Detect 8 bits preamble
    Bits8 = 0x04,
    /// Detect 16 bits preamble
    Bits16 = 0x05,
    /// Detect 24 bits preamble
    Bits24 = 0x06,
    /// Detect 32 bits preamble
    Bits32 = 0x07,
}

impl GfskPreambleDetector {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// GFSK address filtering
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GfskAddressFiltering {
    /// Address filtering disabled
    Disabled = 0x00,
    /// Filter on node address
    Node = 0x01,
    /// Filter on node and broadcast addresses
    NodeAndBroadcast = 0x02,
}

impl GfskAddressFiltering {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// GFSK packet header type
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GfskHeaderType {
    /// Fixed length packet (no length field)
    Fixed = 0x00,
    /// Variable length packet (length in header)
    Variable = 0x01,
}

impl GfskHeaderType {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// GFSK CRC type
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GfskCrcType {
    /// CRC disabled
    Off = 0x01,
    /// 1-byte CRC
    Crc1Byte = 0x00,
    /// 2-byte CRC
    Crc2Bytes = 0x02,
    /// 1-byte CRC, inverted
    Crc1ByteInv = 0x04,
    /// 2-byte CRC, inverted
    Crc2BytesInv = 0x06,
}

impl GfskCrcType {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// GFSK whitening configuration
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum GfskDcFree {
    /// Whitening disabled
    Off = 0x00,
    /// Whitening enabled
    Whitening = 0x01,
}

impl GfskDcFree {
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Maximum length of GFSK sync word in bytes
pub const GFSK_SYNC_WORD_MAX_LENGTH: usize = 8;

/// GFSK modulation parameters
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct GfskModulationParams {
    /// Bitrate in bits per second (600 to 300000)
    pub bitrate_bps: u32,
    /// Pulse shaping filter
    pub pulse_shape: GfskPulseShape,
    /// Receiver bandwidth
    pub bandwidth: GfskBandwidth,
    /// Frequency deviation in Hz
    pub freq_dev_hz: u32,
}

impl Default for GfskModulationParams {
    fn default() -> Self {
        Self {
            bitrate_bps: 50000,
            pulse_shape: GfskPulseShape::Bt1,
            bandwidth: GfskBandwidth::Bw117300,
            freq_dev_hz: 25000,
        }
    }
}

/// GFSK packet parameters
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct GfskPacketParams {
    /// Preamble length in bits (must be multiple of 8)
    pub preamble_length: u16,
    /// Preamble detector length
    pub preamble_detector: GfskPreambleDetector,
    /// Sync word length in bits (0-64, must be multiple of 8)
    pub sync_word_length_bits: u8,
    /// Address filtering mode
    pub address_filtering: GfskAddressFiltering,
    /// Header type (fixed or variable length)
    pub header_type: GfskHeaderType,
    /// Payload length in bytes (for fixed length packets)
    pub payload_length: u8,
    /// CRC type
    pub crc_type: GfskCrcType,
    /// DC-free encoding (whitening)
    pub dc_free: GfskDcFree,
}

impl Default for GfskPacketParams {
    fn default() -> Self {
        Self {
            preamble_length: 32,
            preamble_detector: GfskPreambleDetector::Bits16,
            sync_word_length_bits: 32,
            address_filtering: GfskAddressFiltering::Disabled,
            header_type: GfskHeaderType::Variable,
            payload_length: 255,
            crc_type: GfskCrcType::Crc2Bytes,
            dc_free: GfskDcFree::Whitening,
        }
    }
}

/// Default GFSK sync word (4 bytes)
pub const GFSK_DEFAULT_SYNC_WORD: [u8; 8] = [0xC1, 0x94, 0xC1, 0x94, 0x00, 0x00, 0x00, 0x00];

// =============================================================================
// Radio Statistics Types (from SWDR001 lr11xx_radio.c/h)
// =============================================================================

/// GFSK statistics
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct GfskStats {
    /// Number of received packets
    pub nb_pkt_received: u16,
    /// Number of packets received with CRC error
    pub nb_pkt_crc_error: u16,
    /// Number of packets received with length error
    pub nb_pkt_len_error: u16,
}

/// LoRa statistics
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct LoRaStats {
    /// Number of received packets
    pub nb_pkt_received: u16,
    /// Number of packets received with CRC error
    pub nb_pkt_crc_error: u16,
    /// Number of packets with header error
    pub nb_pkt_header_error: u16,
    /// Number of false sync detected
    pub nb_pkt_false_sync: u16,
}

/// Combined radio statistics
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum RadioStats {
    /// GFSK packet statistics
    Gfsk(GfskStats),
    /// LoRa packet statistics
    LoRa(LoRaStats),
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
            | (self.pll_enable as u8)
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
    Loc = 0x06, // GNSS/WiFi scanning
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
pub fn delay_between_last_bit_sent_and_rx_done_in_us(spreading_factor: SpreadingFactor, bandwidth: Bandwidth) -> u32 {
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
