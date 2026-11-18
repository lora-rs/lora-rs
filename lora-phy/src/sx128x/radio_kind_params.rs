use crate::mod_params::*;

/// Chip variant marker. The SX1280 and SX1281 share the LoRa command set,
/// the SX1281 only lacks the ranging engine.
pub trait Sx128xVariant {}

/// Table 11-23: time base of the SetTx/SetRx timeouts
#[allow(dead_code)]
pub enum PeriodBase {
    _15_625us = 0x00,
    _62_5us = 0x01,
    _1ms = 0x02,
    _4ms = 0x03,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum IrqMask {
    None = 0x0000,
    TxDone = 0x0001,
    RxDone = 0x0002,
    SyncWordValid = 0x0004,
    SyncWordError = 0x0008,
    HeaderValid = 0x0010,
    HeaderError = 0x0020,
    CrcError = 0x0040,
    RangingSlaveResponse = 0x0080,
    RangingSlaveRequestDiscard = 0x0100,
    RangingMasterResultValid = 0x0200,
    RangingMasterTimeout = 0x0400,
    RangingSlaveRequestValid = 0x0800,
    CadDone = 0x1000,
    CadDetected = 0x2000,
    RxTxTimeout = 0x4000,
    PreambleDetectedOrAdvancedRangingDone = 0x8000,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub enum StandbyConfig {
    Rc = 0,
    Xosc = 1,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub enum PacketType {
    Gfsk = 0,
    LoRa = 1,
    Ranging = 2,
    Flrc = 3,
    Ble = 4,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegulatorMode {
    Ldo = 0x00,
    DcDc = 0x01,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub enum CadSymbolNum {
    _1 = 0x00,
    _2 = 0x20,
    _4 = 0x40,
    _8 = 0x60,
    _16 = 0x80,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
pub enum Register {
    // Sheet 1 of 3 (Table 13-1: List of Registers)
    /// Firmware versions 0xB7A9 and 0xB5A9 can be read from register 0x0153
    FirmwareVersions = 0x0153,
    /// Register determining the LNA gain regime (bits 6:7): 3 = high sensitivity mode, 0 = low power mode
    RxGain = 0x0891,
    /// Manual Gain (See Section 4.2)
    ManualGainSetting = 0x0895,
    /// The LNA gain value (See Section 4.2)
    LnaGainValue = 0x089E,
    /// Enable/Disable manual LNA gain control
    LnaGainControl = 0x089F,
    /// dB Attenuation of the peak power during synch address (bits 5:3)
    SynchPeakAttenuation = 0x08C2,
    /// The length of the received LoRa payload
    PayloadLength = 0x0901,
    /// Indicates the LoRa modem header mode: 0 = Header, 1 = No header (bit 7)
    LoRaHeaderMode = 0x0903,
    /// Ranging Master: address of the Slave device to which request is sent (Byte 3)
    RangingRequestAddressByte3 = 0x0912,
    /// Ranging Master: address of the Slave device to which request is sent (Byte 2)
    RangingRequestAddressByte2 = 0x0913,
    /// Ranging Master: address of the Slave device to which request is sent (Byte 1)
    RangingRequestAddressByte1 = 0x0914,
    /// Ranging Master: address of the Slave device to which request is sent (Byte 0)
    RangingRequestAddressByte0 = 0x0915,
    /// Ranging Address when used in Slave and Advanced Ranging mode (Byte 3)
    RangingDeviceAddressByte3 = 0x0916,
    /// Ranging Address when used in Slave and Advanced Ranging mode (Byte 2)
    RangingDeviceAddressByte2 = 0x0917,
    /// Ranging Address when used in Slave and Advanced Ranging mode (Byte 1)
    RangingDeviceAddressByte1 = 0x0918,
    /// Ranging Address when used in Slave and Advanced Ranging mode (Byte 0)
    RangingDeviceAddressByte0 = 0x0919,
    /// The number of ranging samples over which the RSSI evaluated and the results averaged
    RangingFilterWindowSize = 0x091E,
    /// Clears the samples stored in the ranging filter (bit 6)
    ResetRangingFilter = 0x0923,

    // Sheet 2 of 3 (Table 13-1: List of Registers)
    /// Ranging result configuration (bits 4:5)
    RangingResultMux = 0x0924,
    /// SF range selection in LoRa mode
    SfAdditionalConfiguration = 0x0925,
    /// The ranging calibration value (Byte 2)
    RangingCalibrationByte2 = 0x092B,
    /// The ranging calibration value (Byte 1)
    RangingCalibrationByte1 = 0x092C,
    /// The ranging calibration value (Byte 0)
    RangingCalibrationByte0 = 0x092D,
    /// The number of bytes of the Ranging Slave ID that are checked (0: 8 bits, 1: 16 bits, 2: 24 bits, 3: 32 bits)
    RangingIdCheckLength = 0x0931,
    /// Undocumented; SWL2001 writes the IQ polarity here after SetPacketParams (`SX128X_REG_LR_IQ_CONF`)
    LoRaIqPolarity = 0x093B,
    /// Crystal frequency error correction mode (bits 0:2)
    FrequencyErrorCorrection = 0x093C,
    /// Peak-to-noise ratio decision threshold for the CAD
    CadDetPeak = 0x0942,
    /// LoRa sync word value MSB (0x0944)
    LoRaSyncWord0 = 0x0944,
    /// LoRa sync word value LSB (0x0945)
    LoRaSyncWord1 = 0x0945,
    /// CRC present in LoRa incoming packet (bit 4)
    HeaderCrc = 0x0954,
    /// Coding Rate in LoRa incoming packet (bits 4:6)
    CodingRate = 0x0950,
    /// LoRa Frequency error indicator FEI 8:15
    FeiByte1M = 0x0955,
    /// LoRa Frequency error indicator FEI 0:7
    FeiByte0L = 0x0956,
    /// The result of the last ranging exchange (Byte 2)
    RangingResultByte2 = 0x0961,
    /// The result of the last ranging exchange (Byte 1)
    RangingResultByte1 = 0x0962,
    /// The result of the last ranging exchange (Byte 0)
    RangingResultByte0 = 0x0963,
    /// The RSSI value of the last ranging exchange
    RangingRssi = 0x0964,
    /// Set to preserve the ranging result for reading (bit 1)
    FreezeRangingResult = 0x097F,
    /// Preamble length in GFSK and Bluetooth Low Energy compatible (bits 4:6)
    PacketPreambleSettings = 0x09C1,
    /// Data whitening seed for GFSK and Bluetooth Low Energy compatible modulation
    WhiteningInitialValue = 0x09C5,

    // Sheet 3 of 3 (Table 13-1: List of Registers)
    /// CRC Polynomial Definition for GFSK (MSB)
    CrcPolynomialDefinitionMsb = 0x09C6,
    /// CRC Polynomial Definition for GFSK (LSB)
    CrcPolynomialDefinitionLsb = 0x09C7,
    /// CRC Seed for Bluetooth Low Energy compatible modulation (Byte 1)
    CrcPolynomialSeedByte1 = 0x09C8,
    /// CRC Seed for Bluetooth Low Energy compatible modulation (Byte 0)
    CrcPolynomialSeedByte0 = 0x09C9,
    /// The number of sync word bit errors tolerated in FLRC and GFSK modes (bits 0:3)
    SyncAddressControl = 0x09CD,
    /// Sync Word 1 / BLE Access Address (Byte 4)
    SyncAddress1Byte4 = 0x09CE,
    /// Sync Word 1 / BLE Access Address (Byte 3)
    SyncAddress1Byte3 = 0x09CF,
    /// Sync Word 1 / BLE Access Address (Byte 2)
    SyncAddress1Byte2 = 0x09D0,
    /// Sync Word 1 / BLE Access Address (Byte 1)
    SyncAddress1Byte1 = 0x09D1,
    /// Sync Word 1 / BLE Access Address (Byte 0)
    SyncAddress1Byte0 = 0x09D2,
    /// Sync Word 2 (Byte 4)
    SyncAddress2Byte4 = 0x09D3,
    /// Sync Word 2 (Byte 3)
    SyncAddress2Byte3 = 0x09D4,
    /// Sync Word 2 (Byte 2)
    SyncAddress2Byte2 = 0x09D5,
    /// Sync Word 2 (Byte 1)
    SyncAddress2Byte1 = 0x09D6,
    /// Sync Word 2 (Byte 0)
    SyncAddress2Byte0 = 0x09D7,
    /// Sync Word 3 (Byte 4)
    SyncAddress3Byte4 = 0x09D8,
    /// Sync Word 3 (Byte 3)
    SyncAddress3Byte3 = 0x09D9,
    /// Sync Word 3 (Byte 2)
    SyncAddress3Byte2 = 0x09DA,
    /// Sync Word 3 (Byte 1)
    SyncAddress3Byte1 = 0x09DB,
    /// Sync Word 3 (Byte 0)
    SyncAddress3Byte0 = 0x09DC,
    /// Firmware version string (SWL2001 `SX128X_REG_FW_VERSION`)
    VersionString = 0x01F0,
    /// Undocumented; SWL2001 rewrites it after every wakeup (`SX128X_REG_RSSI_SNR_BUGFIX_ADDRESS`)
    RssiSnrBugfix = 0x00EF,
}

impl Register {
    pub fn addr1(self) -> u8 {
        ((self as u16 & 0xFF00) >> 8) as u8
    }
    pub fn addr2(self) -> u8 {
        (self as u16 & 0x00FF) as u8
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum OpCode {
    /// No operation; asserting NSS for it wakes the chip from sleep
    Nop = 0x00,
    // Table 12-1: Transceiver Available Commands

    // Operational Modes Functions
    /// Returns current status of the transceiver
    GetStatus = 0xC0,
    /// Sets transceiver to Sleep mode
    SetSleep = 0x84,
    /// Sets transceiver to Standby mode
    SetStandby = 0x80,
    /// Sets transceiver to Frequency Synthesis (FS) mode
    SetFs = 0xC1,
    /// Sets transceiver to Transmit (Tx) mode
    SetTx = 0x83,
    /// Sets transceiver to Receive (Rx) mode
    SetRx = 0x82,
    /// Sets transceiver to Rx Duty Cycle mode
    SetRxDutyCycle = 0x94,
    /// Sets transceiver to Channel Activity Detection (CAD) mode
    SetCad = 0xC5,
    /// Sets transceiver to Continuous Wave transmission mode
    SetTxContinuousWave = 0xD1,
    /// Sets transceiver to Continuous Preamble transmission mode
    SetTxContinuousPreamble = 0xD2,
    /// Sets auto Frequency Synthesis mode
    SetAutoFS = 0x9E,
    /// Sets auto Transmit mode with specified time
    SetAutoTx = 0x98,

    // Register and Buffer Access Functions
    /// Writes data into radio register memory
    WriteRegister = 0x18,
    /// Reads data from radio register memory
    ReadRegister = 0x19,
    /// Writes data into radio data buffer memory
    WriteBuffer = 0x1A,
    /// Reads data from radio data buffer memory
    ReadBuffer = 0x1B,

    // DIO and Interrupt Control Functions
    /// Configures DIO interrupt masks
    SetDioIrqParams = 0x8D,
    /// Returns current interrupt status
    GetIrqStatus = 0x15,
    /// Clears selected interrupt flags
    ClrIrqStatus = 0x97,

    // RF Modulation and Packet Functions
    /// Sets modem packet type (GFSK, LoRa, Ranging, FLRC, BLE)
    SetPacketType = 0x8A,
    /// Returns current modem packet type
    GetPacketType = 0x03,
    /// Sets RF frequency
    SetRfFrequency = 0x86,
    /// Sets transmit power and ramp time
    SetTxParams = 0x8E,
    /// Sets CAD symbol number
    SetCadParams = 0x88,
    /// Sets base addresses for Tx and Rx data buffer
    SetBufferBaseAddress = 0x8F,
    /// Sets modulation parameters
    SetModulationParams = 0x8B,
    /// Sets packet parameters
    SetPacketParams = 0x8C,
    /// Returns payload length and Rx buffer offset
    GetRxBufferStatus = 0x17,
    /// Returns packet status information
    GetPacketStatus = 0x1D,
    /// Returns instantaneous RSSI
    GetRssiInst = 0x1F,
    /// Sets long preamble mode
    SetLongPreamble = 0x9B,

    // Miscellaneous and Advanced Functions
    /// Sets power regulation mode (LDO or DC-DC)
    SetRegulatorMode = 0x96,
    /// Saves context in Sleep mode
    SetSaveContext = 0xD5,
    /// Sets UART interface speed
    SetUartSpeed = 0x9D,
    /// Sets ranging role (Slave or Master)
    SetRangingRole = 0xA3,
    /// Enables or disables advanced ranging
    SetAdvancedRanging = 0x9A,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
#[allow(clippy::enum_variant_names)]
pub enum RampTime {
    Ramp02Us = 0x00,
    Ramp04Us = 0x20,
    Ramp06Us = 0x40,
    Ramp08Us = 0x60,
    Ramp10Us = 0x80,
    Ramp12Us = 0xA0,
    Ramp16Us = 0xC0,
    Ramp20Us = 0xE0,
}

pub fn spreading_factor_value(spreading_factor: SpreadingFactor) -> Result<u8, RadioError> {
    match spreading_factor {
        SpreadingFactor::_5 => Ok(0x50),
        SpreadingFactor::_6 => Ok(0x60),
        SpreadingFactor::_7 => Ok(0x70),
        SpreadingFactor::_8 => Ok(0x80),
        SpreadingFactor::_9 => Ok(0x90),
        SpreadingFactor::_10 => Ok(0xA0),
        SpreadingFactor::_11 => Ok(0xB0),
        SpreadingFactor::_12 => Ok(0xC0),
    }
}

pub fn bandwidth_value(bw: Bandwidth) -> Result<u8, RadioError> {
    match bw {
        Bandwidth::_200KHz => Ok(0x34),
        Bandwidth::_400KHz => Ok(0x26),
        Bandwidth::_800KHz => Ok(0x18),
        Bandwidth::_1600KHz => Ok(0x0A),
        _ => Err(RadioError::InvalidBandwidthForFrequency),
    }
}

pub fn coding_rate_value(cr: CodingRate) -> Result<u8, RadioError> {
    match cr {
        CodingRate::_4_5 => Ok(0x01),
        CodingRate::_4_6 => Ok(0x02),
        CodingRate::_4_7 => Ok(0x03),
        CodingRate::_4_8 => Ok(0x04),
        CodingRate::Li4_5 => Ok(0x05),
        CodingRate::Li4_6 => Ok(0x06),
        CodingRate::Li4_8 => Ok(0x07),
    }
}
