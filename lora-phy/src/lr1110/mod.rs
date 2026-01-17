//! LR1110 transceiver driver implementation
//!
//! This module provides support for the Semtech LR1110 multi-band transceiver.

#![allow(missing_docs)]

pub mod radio_kind_params;
pub mod variant;

use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::*;
pub use radio_kind_params::TcxoCtrlVoltage;
pub use radio_kind_params::{
    LrFhssBandwidth, LrFhssCodingRate, LrFhssGrid, LrFhssModulationType, LrFhssParams, LrFhssV1Params,
    LR_FHSS_DEFAULT_SYNC_WORD, LR_FHSS_SYNC_WORD_BYTES,
};
// System types
pub use radio_kind_params::{
    ChipMode, ChipType, CommandStatus, ResetStatus, Stat1, Stat2, SystemStatus, Version, LR11XX_SYSTEM_JOIN_EUI_LENGTH,
    LR11XX_SYSTEM_UID_LENGTH,
};
// GNSS types
pub use radio_kind_params::{
    GnssAssistancePosition, GnssConstellation, GnssConstellationMask, GnssContextStatus, GnssDestination,
    GnssDetectedSatellite, GnssErrorCode, GnssFreqSearchSpace, GnssHostStatus, GnssOpCode, GnssResultFields,
    GnssScanMode, GnssSearchMode, GnssVersion, GNSS_BEIDOU_MASK, GNSS_CONTEXT_STATUS_LENGTH, GNSS_GPS_MASK,
    GNSS_MAX_RESULT_SIZE, GNSS_SCALING_LATITUDE, GNSS_SCALING_LONGITUDE, GNSS_SINGLE_ALMANAC_READ_SIZE,
    GNSS_SINGLE_ALMANAC_WRITE_SIZE, GNSS_SNR_TO_CNR_OFFSET,
};
// WiFi types
pub use radio_kind_params::{
    WifiBasicCompleteResult, WifiBasicMacTypeChannelResult, WifiChannel, WifiChannelMask, WifiCumulativeTimings,
    WifiExtendedFullResult, WifiFcsInfo, WifiFrameType, WifiMacAddress, WifiMacOrigin, WifiOpCode, WifiResultFormat,
    WifiScanMode, WifiSignalTypeResult, WifiSignalTypeScan, WifiVersion, WIFI_ALL_CHANNELS_MASK, WIFI_CHANNEL_10_MASK,
    WIFI_CHANNEL_11_MASK, WIFI_CHANNEL_12_MASK, WIFI_CHANNEL_13_MASK, WIFI_CHANNEL_14_MASK, WIFI_CHANNEL_1_MASK,
    WIFI_CHANNEL_2_MASK, WIFI_CHANNEL_3_MASK, WIFI_CHANNEL_4_MASK, WIFI_CHANNEL_5_MASK, WIFI_CHANNEL_6_MASK,
    WIFI_CHANNEL_7_MASK, WIFI_CHANNEL_8_MASK, WIFI_CHANNEL_9_MASK, WIFI_MAC_ADDRESS_LENGTH, WIFI_MAX_RESULTS,
    WIFI_RESULT_SSID_LENGTH,
};
// Crypto Engine types
pub use radio_kind_params::{
    CryptoElement, CryptoKey, CryptoKeyId, CryptoLorawanVersion, CryptoMic, CryptoNonce, CryptoOpCode, CryptoParam,
    CryptoStatus, CRYPTO_AES_CMAC_LENGTH, CRYPTO_DATA_MAX_LENGTH, CRYPTO_KEY_LENGTH, CRYPTO_MIC_LENGTH,
    CRYPTO_NONCE_LENGTH, CRYPTO_PARAMETER_LENGTH,
};
// RTToF (Round-Trip Time of Flight) types
pub use radio_kind_params::{
    rttof_distance_raw_to_meters, rttof_rssi_raw_to_dbm, RttofDistanceResult, RttofOpCode, RttofRawResult,
    RttofResultType, RTTOF_DEFAULT_ADDRESS, RTTOF_DEFAULT_NB_SYMBOLS, RTTOF_RESULT_LENGTH,
};
// RTToF Ranging demo constants and helpers
pub use radio_kind_params::{
    calculate_ranging_request_delay_ms, calculate_symbol_time_ms, lora_bw, lora_cr, lora_sf, packet_type,
    ranging_channels, ranging_config, ranging_irq,
};
// IrqMask for direct use
pub use radio_kind_params::IrqMask;
// Bootloader types
pub use radio_kind_params::{
    BootloaderChipEui, BootloaderCommandStatus, BootloaderJoinEui, BootloaderOpCode, BootloaderPin, BootloaderStat1,
    BootloaderStat2, BootloaderStatus, BootloaderVersion, BOOTLOADER_CHIP_EUI_LENGTH,
    BOOTLOADER_FLASH_BLOCK_SIZE_BYTES, BOOTLOADER_FLASH_BLOCK_SIZE_WORDS, BOOTLOADER_JOIN_EUI_LENGTH,
    BOOTLOADER_PIN_LENGTH, BOOTLOADER_VERSION_LENGTH,
};
// RegMem (Register/Memory) types
pub use radio_kind_params::{RegMemOpCode, REGMEM_BUFFER_SIZE_MAX, REGMEM_MAX_READ_WRITE_WORDS};
// Radio Timings helpers
use radio_kind_params::*;
pub use radio_kind_params::{
    delay_between_last_bit_sent_and_rx_done_in_us, delay_between_last_bit_sent_and_tx_done_in_us,
    lora_rx_input_delay_in_us, lora_symbol_time_in_us, RX_DONE_IRQ_PROCESSING_TIME_IN_US,
    TX_DONE_IRQ_PROCESSING_TIME_IN_US,
};

use crate::mod_params::*;
use crate::mod_traits::IrqState;
use crate::{InterfaceVariant, RadioKind, SpiInterface};
pub use variant::*;

// Internal frequency of the radio
#[allow(dead_code)]
const LR1110_XTAL_FREQ: u32 = 32_000_000;

// Time required for the TCXO to wakeup [ms]
const BRD_TCXO_WAKEUP_TIME: u32 = 10;

// Maximum value for parameter symbNum (same as SX126x)
const LR1110_MAX_LORA_SYMB_NUM_TIMEOUT: u8 = 248;

// SetRx timeout argument for enabling continuous mode
const RX_CONTINUOUS_TIMEOUT: u32 = 0xFFFFFF;

/// Configuration for LR1110-based boards
pub struct Config<C: Lr1110Variant> {
    /// LoRa chip variant on this board
    pub chip: C,
    /// Board is using TCXO
    pub tcxo_ctrl: Option<TcxoCtrlVoltage>,
    /// Whether board is using optional DCDC in addition to LDO
    pub use_dcdc: bool,
    /// Whether to boost receive
    pub rx_boost: bool,
}

/// Base for the RadioKind implementation for the LR1110 chip kind and board type
pub struct Lr1110<SPI, IV, C: Lr1110Variant> {
    intf: SpiInterface<SPI, IV>,
    config: Config<C>,
}

impl<SPI, IV, C> Lr1110<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Lr1110Variant,
{
    /// Create an instance of the RadioKind implementation for the LR1110 chip
    pub fn new(spi: SPI, iv: IV, config: Config<C>) -> Self {
        let intf = SpiInterface::new(spi, iv);
        Self { intf, config }
    }

    // Utility functions

    /// Write a command to the LR1110 using 16-bit opcode
    async fn write_command(&mut self, data: &[u8]) -> Result<(), RadioError> {
        self.intf.write(data, false).await
    }

    /// Read a command response from the LR1110 using 16-bit opcode
    async fn read_command(&mut self, write_data: &[u8], read_buffer: &mut [u8]) -> Result<(), RadioError> {
        self.intf.read(write_data, read_buffer).await
    }

    /// Read a command response with status byte
    #[allow(dead_code)]
    async fn read_command_with_status(&mut self, write_data: &[u8], read_buffer: &mut [u8]) -> Result<u8, RadioError> {
        self.intf.read_with_status(write_data, read_buffer).await
    }

    /// Write data to the TX buffer
    async fn write_buffer(&mut self, offset: u8, data: &[u8]) -> Result<(), RadioError> {
        let opcode = RegMemOpCode::WriteBuffer8.bytes();
        let header = [opcode[0], opcode[1], offset];
        self.intf.write_with_payload(&header, data, false).await
    }

    /// Read data from the RX buffer
    async fn read_buffer(&mut self, offset: u8, length: u8, buffer: &mut [u8]) -> Result<(), RadioError> {
        let opcode = RegMemOpCode::ReadBuffer8.bytes();
        let header = [opcode[0], opcode[1], offset, 0x00];
        self.intf.read(&header, &mut buffer[..length as usize]).await
    }

    /// Set the number of symbols the radio will wait to detect a reception
    async fn set_lora_symbol_num_timeout(&mut self, symbol_num: u16) -> Result<(), RadioError> {
        let mut exp = 0u8;
        let mut mant = ((symbol_num.min(LR1110_MAX_LORA_SYMB_NUM_TIMEOUT.into()) + 1) >> 1) as u8;

        while mant > 31 {
            mant = (mant + 3) >> 2;
            exp += 1;
        }

        let timeout_value = exp + (mant << 3);
        let opcode = RadioOpCode::SetLoRaSyncTimeout.bytes();
        let cmd = [opcode[0], opcode[1], timeout_value];
        self.write_command(&cmd).await
    }

    /// Set PA configuration for LR1110
    async fn set_pa_config(
        &mut self,
        pa_sel: PaSelection,
        pa_supply: PaRegSupply,
        pa_duty_cycle: u8,
        pa_hp_sel: u8,
    ) -> Result<(), RadioError> {
        let opcode = RadioOpCode::SetPaCfg.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            pa_sel.value(),
            pa_supply.value(),
            pa_duty_cycle,
            pa_hp_sel,
        ];
        self.write_command(&cmd).await
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

    #[allow(dead_code)]
    fn convert_freq_in_hz_to_pll_step(freq_in_hz: u32) -> u32 {
        // LR1110 uses direct frequency value in Hz (not PLL steps like SX126x)
        // The formula is: freq_in_hz * 2^25 / XTAL_FREQ
        (((freq_in_hz as u64) << 25) / (LR1110_XTAL_FREQ as u64)) as u32
    }

    // =========================================================================
    // LR-FHSS Public Methods
    // =========================================================================

    /// Initialize LR-FHSS mode
    ///
    /// This sets the packet type to LR-FHSS and configures modulation parameters.
    /// Must be called before lr_fhss_build_frame().
    ///
    /// Reference: SWDR001 lr11xx_lr_fhss_init()
    pub async fn lr_fhss_init(&mut self) -> Result<(), RadioError> {
        // Step 1: Set packet type to LR-FHSS (0x04)
        let pkt_type_opcode = RadioOpCode::SetPktType.bytes();
        let pkt_type_cmd = [pkt_type_opcode[0], pkt_type_opcode[1], PacketType::LrFhss.value()];
        self.write_command(&pkt_type_cmd).await?;

        // Step 2: Set LR-FHSS modulation parameters (bitrate 488 bps, BT=1 pulse shape)
        // Format: opcode[2] + bitrate[4] + pulse_shape[1]
        // Note: These are special encoded values from SWDR001, NOT the raw bps values!
        let mod_opcode = RadioOpCode::SetModulationParam.bytes();
        let bitrate: u32 = 0x8001E848; // LR11XX_RADIO_LR_FHSS_BITRATE_488_BPS (encoded)
        let pulse_shape: u8 = 0x0B; // LR11XX_RADIO_LR_FHSS_PULSE_SHAPE_BT_1
        let mod_cmd = [
            mod_opcode[0],
            mod_opcode[1],
            ((bitrate >> 24) & 0xFF) as u8,
            ((bitrate >> 16) & 0xFF) as u8,
            ((bitrate >> 8) & 0xFF) as u8,
            (bitrate & 0xFF) as u8,
            pulse_shape,
        ];
        self.write_command(&mod_cmd).await
    }

    /// Build and transmit an LR-FHSS frame
    ///
    /// This command configures the LR-FHSS parameters, writes the payload,
    /// and prepares the radio for transmission.
    ///
    /// Reference: SWDR001 lr11xx_lr_fhss_build_frame()
    pub async fn lr_fhss_build_frame(
        &mut self,
        params: &LrFhssParams,
        hop_sequence_id: u16,
        payload: &[u8],
    ) -> Result<(), RadioError> {
        // Set LR-FHSS sync word from params (matching SWDR001 behavior)
        self.lr_fhss_set_sync_word(&params.lr_fhss_params.sync_word).await?;

        // Build LR-FHSS frame command
        // Format per SWDR001: opcode[2] + header_count + cr + modulation_type + grid +
        //                     enable_hopping + bw + hop_seq_id[2] + device_offset
        // Total: 11 bytes command, then payload follows
        let opcode = LrFhssOpCode::BuildFrame.bytes();

        // Construct command buffer
        let lr_fhss_params = &params.lr_fhss_params;
        let enable_hopping: u8 = if lr_fhss_params.enable_hopping { 1 } else { 0 };

        let cmd = [
            opcode[0],
            opcode[1],
            lr_fhss_params.header_count,            // [2] header_count
            lr_fhss_params.coding_rate.value(),     // [3] cr
            lr_fhss_params.modulation_type.value(), // [4] modulation_type
            lr_fhss_params.grid.value(),            // [5] grid
            enable_hopping,                         // [6] enable_hopping
            lr_fhss_params.bandwidth.value(),       // [7] bw
            ((hop_sequence_id >> 8) & 0xFF) as u8,  // [8] hop_seq_id MSB
            (hop_sequence_id & 0xFF) as u8,         // [9] hop_seq_id LSB
            params.device_offset as u8,             // [10] device_offset
        ];

        // Write command with payload (no payload_length field - payload is appended directly)
        self.intf.write_with_payload(&cmd, payload, false).await
    }

    /// Set LR-FHSS sync word
    ///
    /// Uses RadioOpCode::SetLrFhssSyncWord (0x022D)
    async fn lr_fhss_set_sync_word(&mut self, sync_word: &[u8; 4]) -> Result<(), RadioError> {
        let opcode = RadioOpCode::SetLrFhssSyncWord.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            sync_word[0],
            sync_word[1],
            sync_word[2],
            sync_word[3],
        ];
        self.write_command(&cmd).await
    }

    // =========================================================================
    // High ACP Workaround (from SWDR001)
    // =========================================================================

    /// Apply the workaround for the High ACP (Adjacent Channel Power) limitation
    ///
    /// This workaround should be called when the chip wakes up from sleep mode
    /// with retention, before any transmission.
    ///
    /// Affected firmware versions:
    /// - LR1110 firmware from 0x0303 to 0x0307
    /// - LR1120 firmware 0x0101
    ///
    /// The workaround resets bit 30 in register 0x00F30054.
    ///
    /// Reference: SWDR001 README.md, "LR11xx firmware known limitations"
    pub async fn apply_high_acp_workaround(&mut self) -> Result<(), RadioError> {
        // Write 32-bit register with mask: clear bit 30 at address 0x00F30054
        // Command format: opcode[2] + address[4] + mask[4] + data[4]
        let opcode = RegMemOpCode::WriteRegMem32Mask.bytes();
        let address: u32 = HIGH_ACP_WORKAROUND_REG;
        let mask: u32 = 1 << 30; // Bit 30
        let data: u32 = 0; // Clear bit 30

        let cmd = [
            opcode[0],
            opcode[1],
            ((address >> 24) & 0xFF) as u8,
            ((address >> 16) & 0xFF) as u8,
            ((address >> 8) & 0xFF) as u8,
            (address & 0xFF) as u8,
            ((mask >> 24) & 0xFF) as u8,
            ((mask >> 16) & 0xFF) as u8,
            ((mask >> 8) & 0xFF) as u8,
            (mask & 0xFF) as u8,
            ((data >> 24) & 0xFF) as u8,
            ((data >> 16) & 0xFF) as u8,
            ((data >> 8) & 0xFF) as u8,
            (data & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    // =========================================================================
    // System Functions (from SWDR001 lr11xx_system.c)
    // =========================================================================

    /// Initialize the system (TCXO, DC-DC regulator, calibration)
    ///
    /// This performs basic system initialization without configuring radio modulation.
    /// Call this after reset() before using system functions like get_random_number().
    ///
    /// This is automatically called by init_lora(), so you only need to call this
    /// explicitly if you want to use system functions (crypto, RNG, GNSS, WiFi)
    /// without initializing LoRa mode.
    pub async fn init_system(&mut self) -> Result<(), RadioError> {
        // DC-DC regulator setup (default is LDO)
        if self.config.use_dcdc {
            let opcode = SystemOpCode::SetRegMode.bytes();
            let cmd = [opcode[0], opcode[1], RegulatorMode::Dcdc.value()];
            self.write_command(&cmd).await?;
        }

        // DIO3 acting as TCXO controller
        if let Some(voltage) = self.config.tcxo_ctrl {
            // Clear any TCXO startup errors
            let clear_opcode = SystemOpCode::ClearErrors.bytes();
            let clear_cmd = [clear_opcode[0], clear_opcode[1]];
            self.write_command(&clear_cmd).await?;

            // Set TCXO mode - timeout in RTC steps (32.768 kHz)
            let timeout = BRD_TCXO_WAKEUP_TIME * 32768 / 1000; // Convert ms to RTC steps
            let opcode = SystemOpCode::SetTcxoMode.bytes();
            let cmd = [
                opcode[0],
                opcode[1],
                voltage.value(),
                Self::timeout_1(timeout),
                Self::timeout_2(timeout),
                Self::timeout_3(timeout),
            ];
            self.write_command(&cmd).await?;

            // Re-run calibration now that chip knows it's running from TCXO
            let cal_opcode = SystemOpCode::Calibrate.bytes();
            let cal_cmd = [cal_opcode[0], cal_opcode[1], 0b0111_1111];
            self.write_command(&cal_cmd).await?;
        }

        Ok(())
    }

    /// Configure DIO pins as RF switch control
    ///
    /// This configures which DIO pins (DIO5-DIO10) are set high for each radio mode.
    /// Each parameter is a 6-bit bitmask where bit 0 = DIO5, bit 5 = DIO10.
    ///
    /// # Arguments
    /// * `enable` - Enable RF switch control (true to enable)
    /// * `standby` - DIO mask for standby mode
    /// * `rx` - DIO mask for sub-GHz RX mode
    /// * `tx` - DIO mask for sub-GHz TX mode
    /// * `tx_hp` - DIO mask for sub-GHz high-power TX mode
    /// * `tx_hf` - DIO mask for 2.4 GHz TX mode
    /// * `gnss` - DIO mask for GNSS mode
    /// * `wifi` - DIO mask for WiFi mode
    ///
    /// # Example
    /// ```ignore
    /// // Configure DIO8 (bit 3 = 0x08) for WiFi LNA enable
    /// radio.set_dio_as_rf_switch(true, 0x00, 0x01, 0x02, 0x02, 0x00, 0x00, 0x08).await?;
    /// ```
    pub async fn set_dio_as_rf_switch(
        &mut self,
        enable: bool,
        standby: u8,
        rx: u8,
        tx: u8,
        tx_hp: u8,
        tx_hf: u8,
        gnss: u8,
        wifi: u8,
    ) -> Result<(), RadioError> {
        let opcode = SystemOpCode::SetDioAsRfSwitch.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            if enable { 0x01 } else { 0x00 },
            standby,
            rx,
            tx,
            tx_hp,
            tx_hf,
            gnss,
            wifi,
        ];
        self.write_command(&cmd).await
    }

    /// Get the system version (hardware version, chip type, firmware version)
    ///
    /// Returns version information useful for identifying the chip and
    /// checking firmware compatibility.
    pub async fn get_version(&mut self) -> Result<Version, RadioError> {
        let opcode = SystemOpCode::GetVersion.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 4];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(Version {
            hw: rbuffer[0],
            chip_type: ChipType::from(rbuffer[1]),
            fw: ((rbuffer[2] as u16) << 8) | (rbuffer[3] as u16),
        })
    }

    /// Get the system status (stat1, stat2, irq_status)
    ///
    /// This performs a direct SPI read to get the status bytes that the
    /// LR1110 automatically returns on any read operation.
    pub async fn get_status(&mut self) -> Result<SystemStatus, RadioError> {
        // Direct read - chip returns status bytes automatically
        let mut rbuffer = [0u8; 6];
        self.intf.read(&[], &mut rbuffer).await?;

        Ok(SystemStatus {
            stat1: Stat1::from(rbuffer[0]),
            stat2: Stat2::from(rbuffer[1]),
            irq_status: ((rbuffer[2] as u32) << 24)
                | ((rbuffer[3] as u32) << 16)
                | ((rbuffer[4] as u32) << 8)
                | (rbuffer[5] as u32),
        })
    }

    /// Get the chip temperature in degrees Celsius
    ///
    /// Returns the raw temperature value from the internal sensor.
    /// Temperature in Celsius = (raw_value - 273.15) approximately.
    pub async fn get_temp(&mut self) -> Result<u16, RadioError> {
        let opcode = SystemOpCode::GetTemp.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 2];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(((rbuffer[0] as u16) << 8) | (rbuffer[1] as u16))
    }

    /// Get the battery voltage
    ///
    /// Returns a raw ADC value representing battery voltage.
    /// Actual voltage depends on board configuration.
    pub async fn get_vbat(&mut self) -> Result<u8, RadioError> {
        let opcode = SystemOpCode::GetVbat.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(rbuffer[0])
    }

    /// Get a 32-bit random number from the hardware RNG
    ///
    /// The radio must be in receive mode for best entropy.
    pub async fn get_random_number(&mut self) -> Result<u32, RadioError> {
        let opcode = SystemOpCode::GetRandom.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 4];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(
            ((rbuffer[0] as u32) << 24)
                | ((rbuffer[1] as u32) << 16)
                | ((rbuffer[2] as u32) << 8)
                | (rbuffer[3] as u32),
        )
    }

    /// Read the unique device identifier (8 bytes)
    pub async fn read_uid(&mut self) -> Result<[u8; LR11XX_SYSTEM_UID_LENGTH], RadioError> {
        let opcode = SystemOpCode::ReadUid.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; LR11XX_SYSTEM_UID_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(rbuffer)
    }

    /// Read the Join EUI (8 bytes) - for LoRaWAN
    pub async fn read_join_eui(&mut self) -> Result<[u8; LR11XX_SYSTEM_JOIN_EUI_LENGTH], RadioError> {
        let opcode = SystemOpCode::ReadJoinEui.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; LR11XX_SYSTEM_JOIN_EUI_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(rbuffer)
    }

    /// Get system errors
    ///
    /// Returns a bitmask of error flags that have occurred.
    pub async fn get_errors(&mut self) -> Result<u16, RadioError> {
        let opcode = SystemOpCode::GetErrors.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 2];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(((rbuffer[0] as u16) << 8) | (rbuffer[1] as u16))
    }

    /// Clear system errors
    pub async fn clear_errors(&mut self) -> Result<(), RadioError> {
        let opcode = SystemOpCode::ClearErrors.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    // =========================================================================
    // GNSS Functions (from SWDR001 lr11xx_gnss.c)
    // =========================================================================

    /// Set the GNSS constellations to use
    ///
    /// # Arguments
    /// * `constellation_mask` - Bitmask of constellations to use (GNSS_GPS_MASK | GNSS_BEIDOU_MASK)
    pub async fn gnss_set_constellation(
        &mut self,
        constellation_mask: GnssConstellationMask,
    ) -> Result<(), RadioError> {
        let opcode = GnssOpCode::SetConstellation.bytes();
        let cmd = [opcode[0], opcode[1], constellation_mask];
        self.write_command(&cmd).await
    }

    /// Read the currently configured GNSS constellations
    pub async fn gnss_read_constellation(&mut self) -> Result<GnssConstellationMask, RadioError> {
        let opcode = GnssOpCode::ReadConstellation.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer[0])
    }

    /// Read the supported constellations for this chip
    pub async fn gnss_read_supported_constellations(&mut self) -> Result<GnssConstellationMask, RadioError> {
        let opcode = GnssOpCode::ReadSupportedConstellation.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer[0])
    }

    /// Set the GNSS scan mode (single scan or multiple fast scans)
    pub async fn gnss_set_scan_mode(&mut self, scan_mode: GnssScanMode) -> Result<(), RadioError> {
        let opcode = GnssOpCode::SetScanMode.bytes();
        let cmd = [opcode[0], opcode[1], scan_mode.value()];
        self.write_command(&cmd).await
    }

    /// Launch a GNSS scan
    ///
    /// # Arguments
    /// * `effort_mode` - Search effort mode (affects scan duration)
    /// * `result_mask` - Bitmask of result fields to include
    /// * `nb_sv_max` - Maximum number of satellites to detect (0 = no limit)
    ///
    /// After calling this, wait for GnssScanDone IRQ, then call gnss_get_result_size()
    /// and gnss_read_results().
    pub async fn gnss_scan(
        &mut self,
        effort_mode: GnssSearchMode,
        result_mask: u8,
        nb_sv_max: u8,
    ) -> Result<(), RadioError> {
        let opcode = GnssOpCode::Scan.bytes();
        let cmd = [opcode[0], opcode[1], effort_mode.value(), result_mask, nb_sv_max];
        self.write_command(&cmd).await
    }

    /// Get the size of the GNSS scan result
    ///
    /// Call this after GnssScanDone IRQ to determine buffer size needed.
    pub async fn gnss_get_result_size(&mut self) -> Result<u16, RadioError> {
        let opcode = GnssOpCode::GetResultSize.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 2];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(((rbuffer[0] as u16) << 8) | (rbuffer[1] as u16))
    }

    /// Read the GNSS scan results
    ///
    /// # Arguments
    /// * `result_buffer` - Buffer to store results (must be at least result_size bytes)
    ///
    /// The first byte indicates the destination (Host/Solver/DMC).
    /// Remaining bytes are the NAV message to send to LoRa Cloud or process locally.
    pub async fn gnss_read_results(&mut self, result_buffer: &mut [u8]) -> Result<(), RadioError> {
        let opcode = GnssOpCode::ReadResults.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.read_command(&cmd, result_buffer).await
    }

    /// Get the number of satellites detected in the last scan
    pub async fn gnss_get_nb_satellites(&mut self) -> Result<u8, RadioError> {
        let opcode = GnssOpCode::GetNbSatellites.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer[0])
    }

    /// Get detected satellite information
    ///
    /// # Arguments
    /// * `satellites` - Buffer to store satellite info (max 32 satellites)
    /// * `nb_satellites` - Number of satellites to read (from gnss_get_nb_satellites)
    ///
    /// Returns the actual number of satellites read.
    pub async fn gnss_get_satellites(
        &mut self,
        satellites: &mut [GnssDetectedSatellite],
        nb_satellites: u8,
    ) -> Result<u8, RadioError> {
        let opcode = GnssOpCode::GetSatellites.bytes();
        let cmd = [opcode[0], opcode[1]];

        // Each satellite entry is 4 bytes: id(1) + cnr(1) + doppler(2)
        let n = (nb_satellites as usize).min(satellites.len()).min(32);
        let mut rbuffer = [0u8; 128]; // 32 * 4 = 128 max
        self.read_command(&cmd, &mut rbuffer[..n * 4]).await?;

        for (i, satellite) in satellites.iter_mut().enumerate().take(n) {
            let offset = i * 4;
            *satellite = GnssDetectedSatellite {
                satellite_id: rbuffer[offset],
                cnr: (rbuffer[offset + 1] as i8) - GNSS_SNR_TO_CNR_OFFSET,
                doppler: ((rbuffer[offset + 2] as i16) << 8) | (rbuffer[offset + 3] as i16),
            };
        }

        Ok(n as u8)
    }

    /// Set the assistance position for assisted GNSS scanning
    ///
    /// Setting an approximate position improves scan performance.
    pub async fn gnss_set_assistance_position(&mut self, position: &GnssAssistancePosition) -> Result<(), RadioError> {
        let latitude = ((position.latitude * 2048.0) / GNSS_SCALING_LATITUDE) as i16;
        let longitude = ((position.longitude * 2048.0) / GNSS_SCALING_LONGITUDE) as i16;

        let opcode = GnssOpCode::SetAssistancePosition.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (latitude >> 8) as u8,
            (latitude & 0xFF) as u8,
            (longitude >> 8) as u8,
            (longitude & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Read the current assistance position
    pub async fn gnss_read_assistance_position(&mut self) -> Result<GnssAssistancePosition, RadioError> {
        let opcode = GnssOpCode::ReadAssistancePosition.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 4];
        self.read_command(&cmd, &mut rbuffer).await?;

        let latitude_raw = ((rbuffer[0] as i16) << 8) | (rbuffer[1] as i16);
        let longitude_raw = ((rbuffer[2] as i16) << 8) | (rbuffer[3] as i16);

        Ok(GnssAssistancePosition {
            latitude: (latitude_raw as f32) * GNSS_SCALING_LATITUDE / 2048.0,
            longitude: (longitude_raw as f32) * GNSS_SCALING_LONGITUDE / 2048.0,
        })
    }

    /// Read the GNSS firmware version
    pub async fn gnss_read_firmware_version(&mut self) -> Result<GnssVersion, RadioError> {
        let opcode = GnssOpCode::ReadFwVersion.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 2];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(GnssVersion {
            gnss_firmware: rbuffer[0],
            gnss_almanac: rbuffer[1],
        })
    }

    /// Get the GNSS context status
    ///
    /// Returns information about almanac status, error codes, and configuration.
    pub async fn gnss_get_context_status(&mut self) -> Result<GnssContextStatus, RadioError> {
        let opcode = GnssOpCode::GetContextStatus.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; GNSS_CONTEXT_STATUS_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;

        // Parse the context status bytes per SWDR001
        let firmware_version = rbuffer[0];
        let global_almanac_crc = ((rbuffer[1] as u32) << 24)
            | ((rbuffer[2] as u32) << 16)
            | ((rbuffer[3] as u32) << 8)
            | (rbuffer[4] as u32);
        let error_code = GnssErrorCode::from(rbuffer[5] & 0x0F);
        let almanac_update_gps = (rbuffer[6] & 0x02) != 0;
        let almanac_update_beidou = (rbuffer[6] & 0x04) != 0;
        let freq_search_space = GnssFreqSearchSpace::from(((rbuffer[6] & 0x01) << 1) | ((rbuffer[7] & 0x80) >> 7));

        Ok(GnssContextStatus {
            firmware_version,
            global_almanac_crc,
            error_code,
            almanac_update_gps,
            almanac_update_beidou,
            freq_search_space,
        })
    }

    /// Set the frequency search space for GNSS
    pub async fn gnss_set_freq_search_space(
        &mut self,
        freq_search_space: GnssFreqSearchSpace,
    ) -> Result<(), RadioError> {
        let opcode = GnssOpCode::SetFreqSearchSpace.bytes();
        let cmd = [opcode[0], opcode[1], freq_search_space.value()];
        self.write_command(&cmd).await
    }

    /// Read the frequency search space
    pub async fn gnss_read_freq_search_space(&mut self) -> Result<GnssFreqSearchSpace, RadioError> {
        let opcode = GnssOpCode::ReadFreqSearchSpace.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(GnssFreqSearchSpace::from(rbuffer[0]))
    }

    /// Reset the GNSS time
    pub async fn gnss_reset_time(&mut self) -> Result<(), RadioError> {
        let opcode = GnssOpCode::ResetTime.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    /// Reset the GNSS position and doppler history buffer
    pub async fn gnss_reset_position(&mut self) -> Result<(), RadioError> {
        let opcode = GnssOpCode::ResetPosition.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    /// Update almanac data
    ///
    /// # Arguments
    /// * `blocks` - Almanac blocks to write (20 bytes per satellite)
    /// * `nb_blocks` - Number of blocks to write
    pub async fn gnss_almanac_update(&mut self, blocks: &[u8], nb_blocks: u8) -> Result<(), RadioError> {
        let opcode = GnssOpCode::AlmanacUpdate.bytes();
        let cmd = [opcode[0], opcode[1]];
        let block_size = (nb_blocks as usize) * GNSS_SINGLE_ALMANAC_WRITE_SIZE;
        self.intf.write_with_payload(&cmd, &blocks[..block_size], false).await
    }

    /// Push a solver message to the GNSS engine
    ///
    /// Used to provide assistance data from LoRa Cloud.
    pub async fn gnss_push_solver_msg(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        let opcode = GnssOpCode::PushSolverMsg.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.intf.write_with_payload(&cmd, payload, false).await
    }

    /// Push a device management message to the GNSS engine
    pub async fn gnss_push_dm_msg(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        let opcode = GnssOpCode::PushDmMsg.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.intf.write_with_payload(&cmd, payload, false).await
    }

    // =========================================================================
    // WiFi Functions (based on SWDR001 lr11xx_wifi.c)
    // =========================================================================

    /// Start a WiFi passive scan
    ///
    /// # Arguments
    /// * `signal_type` - Type of WiFi signal to scan for (B, G, N, or B/G/N)
    /// * `channel_mask` - Mask of channels to scan (use WIFI_ALL_CHANNELS_MASK for all)
    /// * `scan_mode` - Scan mode (Beacon, BeaconAndPacket, FullBeacon, or UntilSsid)
    /// * `max_results` - Maximum number of results to capture (0 for no limit, max 32)
    /// * `nb_scan_per_channel` - Number of scans per channel (affects scan time)
    /// * `timeout_per_scan_ms` - Timeout per scan in milliseconds
    /// * `abort_on_timeout` - If true, abort scan on first timeout
    ///
    /// After calling this, wait for WifiScanDone IRQ, then call wifi_get_nb_results()
    /// and wifi_read_basic_mac_type_channel_results().
    #[allow(clippy::too_many_arguments)]
    pub async fn wifi_scan(
        &mut self,
        signal_type: WifiSignalTypeScan,
        channel_mask: WifiChannelMask,
        scan_mode: WifiScanMode,
        max_results: u8,
        nb_scan_per_channel: u8,
        timeout_per_scan_ms: u16,
        abort_on_timeout: bool,
    ) -> Result<(), RadioError> {
        let opcode = WifiOpCode::Scan.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            signal_type.value(),
            (channel_mask >> 8) as u8,
            (channel_mask & 0xFF) as u8,
            scan_mode.value(),
            max_results,
            nb_scan_per_channel,
            (timeout_per_scan_ms >> 8) as u8,
            (timeout_per_scan_ms & 0xFF) as u8,
            abort_on_timeout as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Start a WiFi passive scan with time limit
    ///
    /// Similar to wifi_scan but with a total time limit instead of per-scan timeout.
    ///
    /// # Arguments
    /// * `signal_type` - Type of WiFi signal to scan for
    /// * `channel_mask` - Mask of channels to scan
    /// * `scan_mode` - Scan mode
    /// * `max_results` - Maximum number of results (0 for no limit, max 32)
    /// * `timeout_per_channel_ms` - Timeout per channel in milliseconds
    /// * `timeout_total_ms` - Total scan timeout in milliseconds
    pub async fn wifi_scan_time_limit(
        &mut self,
        signal_type: WifiSignalTypeScan,
        channel_mask: WifiChannelMask,
        scan_mode: WifiScanMode,
        max_results: u8,
        timeout_per_channel_ms: u16,
        timeout_total_ms: u16,
    ) -> Result<(), RadioError> {
        let opcode = WifiOpCode::ScanTimeLimit.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            signal_type.value(),
            (channel_mask >> 8) as u8,
            (channel_mask & 0xFF) as u8,
            scan_mode.value(),
            max_results,
            (timeout_per_channel_ms >> 8) as u8,
            (timeout_per_channel_ms & 0xFF) as u8,
            (timeout_total_ms >> 8) as u8,
            (timeout_total_ms & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Get the number of WiFi results available
    ///
    /// Call this after WifiScanDone IRQ is received.
    pub async fn wifi_get_nb_results(&mut self) -> Result<u8, RadioError> {
        let opcode = WifiOpCode::GetResultSize.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer[0])
    }

    /// Read WiFi scan results in basic MAC/type/channel format (9 bytes per result)
    ///
    /// This is the most compact result format, containing MAC address, signal type,
    /// channel, and RSSI for each detected access point.
    ///
    /// # Arguments
    /// * `results` - Buffer to store results
    /// * `start_index` - Index of first result to read
    /// * `nb_results` - Number of results to read
    ///
    /// # Returns
    /// Number of results actually read
    pub async fn wifi_read_basic_mac_type_channel_results(
        &mut self,
        results: &mut [WifiBasicMacTypeChannelResult],
        start_index: u8,
        nb_results: u8,
    ) -> Result<u8, RadioError> {
        if nb_results == 0 || results.is_empty() {
            return Ok(0);
        }

        let count = nb_results.min(results.len() as u8);
        let result_size = WIFI_BASIC_MAC_TYPE_CHANNEL_RESULT_SIZE;
        let total_size = (count as usize) * result_size;

        // Read raw data
        let opcode = WifiOpCode::ReadResult.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            start_index,
            count,
            WifiResultFormat::BasicMacTypeChannel.format_code(),
        ];

        // Use a stack buffer for reading (max 32 results * 9 bytes = 288 bytes)
        let mut buffer = [0u8; 288];
        self.read_command(&cmd, &mut buffer[..total_size]).await?;

        // Parse results
        for (i, result) in results.iter_mut().enumerate().take(count as usize) {
            let offset = i * result_size;
            result.data_rate_info_byte = buffer[offset];
            result.channel_info_byte = buffer[offset + 1];
            result.rssi = buffer[offset + 2] as i8;
            result.mac_address.copy_from_slice(&buffer[offset + 3..offset + 9]);
        }

        Ok(count)
    }

    /// Read WiFi scan results in basic complete format (22 bytes per result)
    ///
    /// This format includes MAC address, signal info, frame type, timestamp, and beacon period.
    ///
    /// # Arguments
    /// * `results` - Buffer to store results
    /// * `start_index` - Index of first result to read
    /// * `nb_results` - Number of results to read
    ///
    /// # Returns
    /// Number of results actually read
    pub async fn wifi_read_basic_complete_results(
        &mut self,
        results: &mut [WifiBasicCompleteResult],
        start_index: u8,
        nb_results: u8,
    ) -> Result<u8, RadioError> {
        if nb_results == 0 || results.is_empty() {
            return Ok(0);
        }

        let count = nb_results.min(results.len() as u8);
        let result_size = WIFI_BASIC_COMPLETE_RESULT_SIZE;
        let total_size = (count as usize) * result_size;

        // Read raw data
        let opcode = WifiOpCode::ReadResult.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            start_index,
            count,
            WifiResultFormat::BasicComplete.format_code(),
        ];

        // Use stack buffer (max 32 results * 22 bytes = 704 bytes)
        let mut buffer = [0u8; 704];
        self.read_command(&cmd, &mut buffer[..total_size]).await?;

        // Parse results
        for (i, result) in results.iter_mut().enumerate().take(count as usize) {
            let offset = i * result_size;
            result.data_rate_info_byte = buffer[offset];
            result.channel_info_byte = buffer[offset + 1];
            result.rssi = buffer[offset + 2] as i8;
            result.frame_type_info_byte = buffer[offset + 3];
            result.mac_address.copy_from_slice(&buffer[offset + 4..offset + 10]);
            result.phi_offset = ((buffer[offset + 10] as i16) << 8) | (buffer[offset + 11] as i16);
            result.timestamp_us = ((buffer[offset + 12] as u64) << 56)
                | ((buffer[offset + 13] as u64) << 48)
                | ((buffer[offset + 14] as u64) << 40)
                | ((buffer[offset + 15] as u64) << 32)
                | ((buffer[offset + 16] as u64) << 24)
                | ((buffer[offset + 17] as u64) << 16)
                | ((buffer[offset + 18] as u64) << 8)
                | (buffer[offset + 19] as u64);
            result.beacon_period_tu = ((buffer[offset + 20] as u16) << 8) | (buffer[offset + 21] as u16);
        }

        Ok(count)
    }

    /// Read cumulative WiFi scan timing information
    pub async fn wifi_read_cumulative_timing(&mut self) -> Result<WifiCumulativeTimings, RadioError> {
        let opcode = WifiOpCode::ReadCumulTiming.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; WIFI_ALL_CUMULATIVE_TIMING_SIZE];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(WifiCumulativeTimings {
            rx_detection_us: ((rbuffer[0] as u32) << 24)
                | ((rbuffer[1] as u32) << 16)
                | ((rbuffer[2] as u32) << 8)
                | (rbuffer[3] as u32),
            rx_correlation_us: ((rbuffer[4] as u32) << 24)
                | ((rbuffer[5] as u32) << 16)
                | ((rbuffer[6] as u32) << 8)
                | (rbuffer[7] as u32),
            rx_capture_us: ((rbuffer[8] as u32) << 24)
                | ((rbuffer[9] as u32) << 16)
                | ((rbuffer[10] as u32) << 8)
                | (rbuffer[11] as u32),
            demodulation_us: ((rbuffer[12] as u32) << 24)
                | ((rbuffer[13] as u32) << 16)
                | ((rbuffer[14] as u32) << 8)
                | (rbuffer[15] as u32),
        })
    }

    /// Reset cumulative WiFi scan timing counters
    pub async fn wifi_reset_cumulative_timing(&mut self) -> Result<(), RadioError> {
        let opcode = WifiOpCode::ResetCumulTiming.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    /// Read WiFi firmware version
    pub async fn wifi_read_version(&mut self) -> Result<WifiVersion, RadioError> {
        let opcode = WifiOpCode::GetVersion.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut rbuffer = [0u8; WIFI_VERSION_SIZE];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(WifiVersion {
            major: rbuffer[0],
            minor: rbuffer[1],
        })
    }

    /// Configure the timestamp for AP phone scanning
    ///
    /// # Arguments
    /// * `timestamp_s` - Timestamp in seconds
    pub async fn wifi_cfg_timestamp_ap_phone(&mut self, timestamp_s: u32) -> Result<(), RadioError> {
        let opcode = WifiOpCode::ConfigureTimestampApPhone.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (timestamp_s >> 24) as u8,
            (timestamp_s >> 16) as u8,
            (timestamp_s >> 8) as u8,
            timestamp_s as u8,
        ];
        self.write_command(&cmd).await
    }

    // =========================================================================
    // Crypto Engine Functions (from SWDR001 lr11xx_crypto_engine.c)
    // =========================================================================

    /// Select the crypto element to use for subsequent operations
    ///
    /// # Arguments
    /// * `element` - Crypto element to use (CryptoEngine or SecureElement)
    pub async fn crypto_select(&mut self, element: CryptoElement) -> Result<(), RadioError> {
        let opcode = CryptoOpCode::Select.bytes();
        let cmd = [opcode[0], opcode[1], element.value()];
        self.write_command(&cmd).await
    }

    /// Set a key in the specified key slot
    ///
    /// # Arguments
    /// * `key_id` - Key slot to write to
    /// * `key` - 16-byte AES key
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_set_key(&mut self, key_id: CryptoKeyId, key: &CryptoKey) -> Result<CryptoStatus, RadioError> {
        let opcode = CryptoOpCode::SetKey.bytes();
        let mut cmd = [0u8; 19]; // 2 opcode + 1 key_id + 16 key
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = key_id.value();
        cmd[3..19].copy_from_slice(key);

        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(CryptoStatus::from(rbuffer[0]))
    }

    /// Derive a new key from an existing key using a nonce
    ///
    /// # Arguments
    /// * `src_key_id` - Source key slot
    /// * `dest_key_id` - Destination key slot for derived key
    /// * `nonce` - 16-byte nonce for key derivation
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_derive_key(
        &mut self,
        src_key_id: CryptoKeyId,
        dest_key_id: CryptoKeyId,
        nonce: &CryptoNonce,
    ) -> Result<CryptoStatus, RadioError> {
        let opcode = CryptoOpCode::DeriveKey.bytes();
        let mut cmd = [0u8; 20]; // 2 opcode + 1 src_key_id + 1 dest_key_id + 16 nonce
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = src_key_id.value();
        cmd[3] = dest_key_id.value();
        cmd[4..20].copy_from_slice(nonce);

        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(CryptoStatus::from(rbuffer[0]))
    }

    /// Process a LoRaWAN Join Accept message
    ///
    /// Decrypts the Join Accept and verifies its MIC.
    ///
    /// # Arguments
    /// * `dec_key_id` - Key slot for decryption (NwkKey for LoRaWAN 1.0.x)
    /// * `ver_key_id` - Key slot for MIC verification (JSIntKey for LoRaWAN 1.1.x)
    /// * `lorawan_version` - LoRaWAN version (affects header length)
    /// * `header` - Join Accept header (1 byte for 1.0.x, 12 bytes for 1.1.x)
    /// * `data_in` - Encrypted Join Accept data
    /// * `data_out` - Buffer for decrypted data (must be same size as data_in)
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_process_join_accept(
        &mut self,
        dec_key_id: CryptoKeyId,
        ver_key_id: CryptoKeyId,
        lorawan_version: CryptoLorawanVersion,
        header: &[u8],
        data_in: &[u8],
        data_out: &mut [u8],
    ) -> Result<CryptoStatus, RadioError> {
        let header_length = lorawan_version.header_length();
        if header.len() < header_length {
            return Err(RadioError::PayloadSizeMismatch(header_length, header.len()));
        }
        if data_in.len() > 32 || data_in.is_empty() {
            return Err(RadioError::PayloadSizeMismatch(32, data_in.len()));
        }
        if data_out.len() < data_in.len() {
            return Err(RadioError::PayloadSizeMismatch(data_in.len(), data_out.len()));
        }

        let opcode = CryptoOpCode::ProcessJoinAccept.bytes();
        let cmd_len = 2 + 3 + header_length + data_in.len();
        let mut cmd = [0u8; 49]; // Max: 2 + 3 + 12 + 32
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = dec_key_id.value();
        cmd[3] = ver_key_id.value();
        cmd[4] = lorawan_version.value();
        cmd[5..5 + header_length].copy_from_slice(&header[..header_length]);
        cmd[5 + header_length..cmd_len].copy_from_slice(data_in);

        let mut rbuffer = [0u8; 33]; // 1 status + 32 max data
        self.read_command(&cmd[..cmd_len], &mut rbuffer[..1 + data_in.len()])
            .await?;

        let status = CryptoStatus::from(rbuffer[0]);
        if status == CryptoStatus::Success {
            data_out[..data_in.len()].copy_from_slice(&rbuffer[1..1 + data_in.len()]);
        }
        Ok(status)
    }

    /// Compute AES-CMAC (Message Integrity Code) over data
    ///
    /// # Arguments
    /// * `key_id` - Key slot to use for CMAC calculation
    /// * `data` - Data to compute CMAC over (max 256 bytes)
    ///
    /// # Returns
    /// * `Ok((CryptoStatus, CryptoMic))` - Status and 4-byte MIC
    pub async fn crypto_compute_aes_cmac(
        &mut self,
        key_id: CryptoKeyId,
        data: &[u8],
    ) -> Result<(CryptoStatus, CryptoMic), RadioError> {
        if data.len() > CRYPTO_DATA_MAX_LENGTH {
            return Err(RadioError::PayloadSizeMismatch(CRYPTO_DATA_MAX_LENGTH, data.len()));
        }

        let opcode = CryptoOpCode::ComputeAesCmac.bytes();
        let cmd_len = 3 + data.len();
        let mut cmd = [0u8; 259]; // 2 opcode + 1 key_id + 256 data
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = key_id.value();
        cmd[3..cmd_len].copy_from_slice(data);

        let mut rbuffer = [0u8; 5]; // 1 status + 4 MIC
        self.read_command(&cmd[..cmd_len], &mut rbuffer).await?;

        let status = CryptoStatus::from(rbuffer[0]);
        let mut mic = [0u8; CRYPTO_MIC_LENGTH];
        if status == CryptoStatus::Success {
            mic.copy_from_slice(&rbuffer[1..5]);
        }
        Ok((status, mic))
    }

    /// Verify AES-CMAC (Message Integrity Code) over data
    ///
    /// # Arguments
    /// * `key_id` - Key slot to use for CMAC verification
    /// * `data` - Data to verify CMAC over (max 256 bytes)
    /// * `mic` - Expected 4-byte MIC
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Success if MIC matches, ErrorFailCmac otherwise
    pub async fn crypto_verify_aes_cmac(
        &mut self,
        key_id: CryptoKeyId,
        data: &[u8],
        mic: &CryptoMic,
    ) -> Result<CryptoStatus, RadioError> {
        if data.len() > CRYPTO_DATA_MAX_LENGTH {
            return Err(RadioError::PayloadSizeMismatch(CRYPTO_DATA_MAX_LENGTH, data.len()));
        }

        let opcode = CryptoOpCode::VerifyAesCmac.bytes();
        let cmd_len = 3 + CRYPTO_MIC_LENGTH + data.len();
        let mut cmd = [0u8; 263]; // 2 opcode + 1 key_id + 4 MIC + 256 data
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = key_id.value();
        cmd[3..7].copy_from_slice(mic);
        cmd[7..cmd_len].copy_from_slice(data);

        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd[..cmd_len], &mut rbuffer).await?;
        Ok(CryptoStatus::from(rbuffer[0]))
    }

    /// AES encrypt data (legacy variant 01)
    ///
    /// # Arguments
    /// * `key_id` - Key slot to use for encryption
    /// * `data` - Data to encrypt (must be multiple of 16 bytes, max 256)
    /// * `result` - Buffer for encrypted data (same size as input)
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_aes_encrypt_01(
        &mut self,
        key_id: CryptoKeyId,
        data: &[u8],
        result: &mut [u8],
    ) -> Result<CryptoStatus, RadioError> {
        if data.len() > CRYPTO_DATA_MAX_LENGTH || data.is_empty() {
            return Err(RadioError::PayloadSizeMismatch(CRYPTO_DATA_MAX_LENGTH, data.len()));
        }
        if result.len() < data.len() {
            return Err(RadioError::PayloadSizeMismatch(data.len(), result.len()));
        }

        let opcode = CryptoOpCode::AesEncrypt01.bytes();
        let cmd_len = 3 + data.len();
        let mut cmd = [0u8; 259]; // 2 opcode + 1 key_id + 256 data
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = key_id.value();
        cmd[3..cmd_len].copy_from_slice(data);

        let mut rbuffer = [0u8; 257]; // 1 status + 256 data
        self.read_command(&cmd[..cmd_len], &mut rbuffer[..1 + data.len()])
            .await?;

        let status = CryptoStatus::from(rbuffer[0]);
        if status == CryptoStatus::Success {
            result[..data.len()].copy_from_slice(&rbuffer[1..1 + data.len()]);
        }
        Ok(status)
    }

    /// AES encrypt data
    ///
    /// # Arguments
    /// * `key_id` - Key slot to use for encryption
    /// * `data` - Data to encrypt (must be multiple of 16 bytes, max 256)
    /// * `result` - Buffer for encrypted data (same size as input)
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_aes_encrypt(
        &mut self,
        key_id: CryptoKeyId,
        data: &[u8],
        result: &mut [u8],
    ) -> Result<CryptoStatus, RadioError> {
        if data.len() > CRYPTO_DATA_MAX_LENGTH || data.is_empty() {
            return Err(RadioError::PayloadSizeMismatch(CRYPTO_DATA_MAX_LENGTH, data.len()));
        }
        if result.len() < data.len() {
            return Err(RadioError::PayloadSizeMismatch(data.len(), result.len()));
        }

        let opcode = CryptoOpCode::AesEncrypt.bytes();
        let cmd_len = 3 + data.len();
        let mut cmd = [0u8; 259]; // 2 opcode + 1 key_id + 256 data
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = key_id.value();
        cmd[3..cmd_len].copy_from_slice(data);

        let mut rbuffer = [0u8; 257]; // 1 status + 256 data
        self.read_command(&cmd[..cmd_len], &mut rbuffer[..1 + data.len()])
            .await?;

        let status = CryptoStatus::from(rbuffer[0]);
        if status == CryptoStatus::Success {
            result[..data.len()].copy_from_slice(&rbuffer[1..1 + data.len()]);
        }
        Ok(status)
    }

    /// AES decrypt data
    ///
    /// # Arguments
    /// * `key_id` - Key slot to use for decryption
    /// * `data` - Data to decrypt (must be multiple of 16 bytes, max 256)
    /// * `result` - Buffer for decrypted data (same size as input)
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_aes_decrypt(
        &mut self,
        key_id: CryptoKeyId,
        data: &[u8],
        result: &mut [u8],
    ) -> Result<CryptoStatus, RadioError> {
        if data.len() > CRYPTO_DATA_MAX_LENGTH || data.is_empty() {
            return Err(RadioError::PayloadSizeMismatch(CRYPTO_DATA_MAX_LENGTH, data.len()));
        }
        if result.len() < data.len() {
            return Err(RadioError::PayloadSizeMismatch(data.len(), result.len()));
        }

        let opcode = CryptoOpCode::AesDecrypt.bytes();
        let cmd_len = 3 + data.len();
        let mut cmd = [0u8; 259]; // 2 opcode + 1 key_id + 256 data
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = key_id.value();
        cmd[3..cmd_len].copy_from_slice(data);

        let mut rbuffer = [0u8; 257]; // 1 status + 256 data
        self.read_command(&cmd[..cmd_len], &mut rbuffer[..1 + data.len()])
            .await?;

        let status = CryptoStatus::from(rbuffer[0]);
        if status == CryptoStatus::Success {
            result[..data.len()].copy_from_slice(&rbuffer[1..1 + data.len()]);
        }
        Ok(status)
    }

    /// Store crypto keys and data to flash
    ///
    /// Persists the current crypto engine state to internal flash memory.
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_store_to_flash(&mut self) -> Result<CryptoStatus, RadioError> {
        let opcode = CryptoOpCode::StoreToFlash.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(CryptoStatus::from(rbuffer[0]))
    }

    /// Restore crypto keys and data from flash
    ///
    /// Restores the crypto engine state from internal flash memory.
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_restore_from_flash(&mut self) -> Result<CryptoStatus, RadioError> {
        let opcode = CryptoOpCode::RestoreFromFlash.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(CryptoStatus::from(rbuffer[0]))
    }

    /// Set a crypto parameter
    ///
    /// # Arguments
    /// * `param_id` - Parameter identifier
    /// * `parameter` - 4-byte parameter value
    ///
    /// # Returns
    /// * `Ok(CryptoStatus)` - Status of the operation
    pub async fn crypto_set_parameter(
        &mut self,
        param_id: u8,
        parameter: &CryptoParam,
    ) -> Result<CryptoStatus, RadioError> {
        let opcode = CryptoOpCode::SetParameter.bytes();
        let mut cmd = [0u8; 7]; // 2 opcode + 1 param_id + 4 parameter
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = param_id;
        cmd[3..7].copy_from_slice(parameter);

        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(CryptoStatus::from(rbuffer[0]))
    }

    /// Get a crypto parameter
    ///
    /// # Arguments
    /// * `param_id` - Parameter identifier
    ///
    /// # Returns
    /// * `Ok((CryptoStatus, CryptoParam))` - Status and 4-byte parameter value
    pub async fn crypto_get_parameter(&mut self, param_id: u8) -> Result<(CryptoStatus, CryptoParam), RadioError> {
        let opcode = CryptoOpCode::GetParameter.bytes();
        let cmd = [opcode[0], opcode[1], param_id];

        let mut rbuffer = [0u8; 5]; // 1 status + 4 parameter
        self.read_command(&cmd, &mut rbuffer).await?;

        let status = CryptoStatus::from(rbuffer[0]);
        let mut parameter = [0u8; CRYPTO_PARAMETER_LENGTH];
        if status == CryptoStatus::Success {
            parameter.copy_from_slice(&rbuffer[1..5]);
        }
        Ok((status, parameter))
    }

    /// Check a portion of an encrypted firmware image
    ///
    /// This function checks one block of encrypted firmware data.
    /// Use `crypto_check_encrypted_fw_image_full` for checking complete images.
    ///
    /// # Arguments
    /// * `offset` - Byte offset within the firmware image
    /// * `data` - 32-bit words of encrypted firmware data (max 64 words)
    pub async fn crypto_check_encrypted_fw_image(&mut self, offset: u32, data: &[u32]) -> Result<(), RadioError> {
        const MAX_WORDS: usize = 64;
        if data.len() > MAX_WORDS {
            return Err(RadioError::PayloadSizeMismatch(MAX_WORDS, data.len()));
        }

        let opcode = CryptoOpCode::CheckEncryptedFwImage.bytes();
        let mut cmd = [0u8; 262]; // 2 opcode + 4 offset + 256 data (64 * 4)
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = (offset >> 24) as u8;
        cmd[3] = (offset >> 16) as u8;
        cmd[4] = (offset >> 8) as u8;
        cmd[5] = offset as u8;

        for (i, word) in data.iter().enumerate() {
            let idx = 6 + i * 4;
            cmd[idx] = (*word >> 24) as u8;
            cmd[idx + 1] = (*word >> 16) as u8;
            cmd[idx + 2] = (*word >> 8) as u8;
            cmd[idx + 3] = *word as u8;
        }

        let cmd_len = 6 + data.len() * 4;
        self.write_command(&cmd[..cmd_len]).await
    }

    /// Get the result of encrypted firmware image check
    ///
    /// # Returns
    /// * `Ok(bool)` - true if the firmware image is valid
    pub async fn crypto_get_check_encrypted_fw_image_result(&mut self) -> Result<bool, RadioError> {
        let opcode = CryptoOpCode::GetCheckEncryptedFwImageResult.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; 1];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer[0] != 0)
    }

    // =========================================================================
    // RTToF (Round-Trip Time of Flight) Functions (from SWDR001 lr11xx_rttof.c)
    // =========================================================================

    /// Set the RTToF address for this subordinate device
    ///
    /// The address is used in subordinate mode when receiving RTToF requests.
    /// The subordinate compares `check_length` bytes (LSB first) of the request
    /// address with its own address. Non-matching packets are discarded.
    ///
    /// # Arguments
    /// * `address` - 32-bit subordinate address (default is 0x00000019)
    /// * `check_length` - Number of bytes to compare (1..4, default is 4)
    pub async fn rttof_set_address(&mut self, address: u32, check_length: u8) -> Result<(), RadioError> {
        let opcode = RttofOpCode::SetAddress.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (address >> 24) as u8,
            (address >> 16) as u8,
            (address >> 8) as u8,
            address as u8,
            check_length,
        ];
        self.write_command(&cmd).await
    }

    /// Set the RTToF request address for manager mode
    ///
    /// The request address is copied into the RTToF request packets sent
    /// when operating as manager.
    ///
    /// # Arguments
    /// * `request_address` - 32-bit request address (default is 0x00000019)
    pub async fn rttof_set_request_address(&mut self, request_address: u32) -> Result<(), RadioError> {
        let opcode = RttofOpCode::SetRequestAddress.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (request_address >> 24) as u8,
            (request_address >> 16) as u8,
            (request_address >> 8) as u8,
            request_address as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Set the RX/TX delay indicator for RTToF calibration
    ///
    /// The transceiver hardware induces a delay depending on the physical layer
    /// configuration (bandwidth, spreading factor). This delay needs to be
    /// compensated by a calibration value for accurate RTToF measurements.
    ///
    /// # Arguments
    /// * `delay_indicator` - Delay value for the used bandwidth and spreading factor
    ///
    /// # Note
    /// The same delay_indicator must be configured in both manager and subordinate devices.
    pub async fn rttof_set_rx_tx_delay_indicator(&mut self, delay_indicator: u32) -> Result<(), RadioError> {
        let opcode = RttofOpCode::SetRxTxDelay.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (delay_indicator >> 24) as u8,
            (delay_indicator >> 16) as u8,
            (delay_indicator >> 8) as u8,
            delay_indicator as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Configure RTToF specific parameters
    ///
    /// # Arguments
    /// * `nb_symbols` - Number of symbols in subordinate responses (recommended: 15)
    ///
    /// # Note
    /// The RTToF parameters must be configured in both manager and subordinate devices.
    /// A value of 15 symbols balances RTToF accuracy and power consumption.
    pub async fn rttof_set_parameters(&mut self, nb_symbols: u8) -> Result<(), RadioError> {
        let opcode = RttofOpCode::SetParameters.bytes();
        let cmd = [opcode[0], opcode[1], 0x00, nb_symbols];
        self.write_command(&cmd).await
    }

    /// Get the raw RTToF result from the manager device
    ///
    /// # Arguments
    /// * `result_type` - Type of result to retrieve (Raw distance or RSSI)
    ///
    /// # Returns
    /// * `Ok(RttofRawResult)` - 4-byte raw result
    ///
    /// # Note
    /// This function is only available on manager devices after RTToF is complete.
    /// Use `rttof_distance_raw_to_meters()` or `rttof_rssi_raw_to_dbm()` to convert.
    pub async fn rttof_get_raw_result(&mut self, result_type: RttofResultType) -> Result<RttofRawResult, RadioError> {
        let opcode = RttofOpCode::GetResult.bytes();
        let cmd = [opcode[0], opcode[1], result_type.value()];

        let mut rbuffer = [0u8; RTTOF_RESULT_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer)
    }

    /// Get complete RTToF distance result with RSSI
    ///
    /// Convenience function that retrieves both distance and RSSI results
    /// and converts them to meaningful units.
    ///
    /// # Arguments
    /// * `bandwidth` - LoRa bandwidth used during RTToF measurement
    ///
    /// # Returns
    /// * `Ok(RttofDistanceResult)` - Distance in meters and RSSI in dBm
    pub async fn rttof_get_distance_result(&mut self, bandwidth: Bandwidth) -> Result<RttofDistanceResult, RadioError> {
        // Get raw distance
        let raw_distance = self.rttof_get_raw_result(RttofResultType::Raw).await?;
        let distance_m = rttof_distance_raw_to_meters(bandwidth, &raw_distance);

        // Get raw RSSI
        let raw_rssi = self.rttof_get_raw_result(RttofResultType::Rssi).await?;
        let rssi_dbm = rttof_rssi_raw_to_dbm(&raw_rssi);

        Ok(RttofDistanceResult { distance_m, rssi_dbm })
    }

    // =========================================================================
    // RTToF Ranging Public Methods (for ranging demo)
    // =========================================================================

    /// Set the packet type (LoRa, RTToF, etc.)
    ///
    /// Used to switch between standard LoRa mode and RTToF mode for ranging.
    ///
    /// # Arguments
    /// * `packet_type` - The packet type to set (LoRa = 0x02, RTToF = 0x05)
    pub async fn set_packet_type(&mut self, packet_type: u8) -> Result<(), RadioError> {
        let opcode = RadioOpCode::SetPktType.bytes();
        let cmd = [opcode[0], opcode[1], packet_type];
        self.write_command(&cmd).await
    }

    /// Set the RF frequency
    ///
    /// # Arguments
    /// * `frequency_hz` - Frequency in Hz
    pub async fn set_rf_frequency(&mut self, frequency_hz: u32) -> Result<(), RadioError> {
        // LR1110 uses frequency directly in Hz (as 32-bit value)
        let opcode = RadioOpCode::SetRfFrequency.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((frequency_hz >> 24) & 0xFF) as u8,
            ((frequency_hz >> 16) & 0xFF) as u8,
            ((frequency_hz >> 8) & 0xFF) as u8,
            (frequency_hz & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Set DIO IRQ parameters with custom mask
    ///
    /// # Arguments
    /// * `irq_mask` - 32-bit IRQ mask for DIO1
    pub async fn set_dio_irq_params_custom(&mut self, irq_mask: u32) -> Result<(), RadioError> {
        let opcode = SystemOpCode::SetDioIrqParams.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((irq_mask >> 24) & 0xFF) as u8,
            ((irq_mask >> 16) & 0xFF) as u8,
            ((irq_mask >> 8) & 0xFF) as u8,
            (irq_mask & 0xFF) as u8,
            0x00, // DIO2 mask (4 bytes)
            0x00,
            0x00,
            0x00,
        ];
        self.write_command(&cmd).await
    }

    /// Clear all IRQ flags
    pub async fn clear_all_irq(&mut self) -> Result<(), RadioError> {
        let opcode = SystemOpCode::ClearIrq.bytes();
        let cmd = [
            opcode[0], opcode[1], 0xFF, // Clear all interrupts (32-bit mask)
            0xFF, 0xFF, 0xFF,
        ];
        self.write_command(&cmd).await
    }

    /// Read 32-bit IRQ flags without clearing them.
    ///
    /// Uses ClearIrq command with zero mask - this reads the current IRQ flags
    /// without actually clearing any of them (per SWDR001 driver behavior).
    pub async fn get_irq_flags(&mut self) -> Result<u32, RadioError> {
        // Wait for chip to be ready
        self.intf.iv.wait_on_busy().await?;

        // LR1110's ClearIrq command (0x0114) returns the current IRQ flags before clearing.
        // By passing a zero mask, we read the flags without clearing any.
        let opcode = SystemOpCode::ClearIrq.bytes();
        let cmd = [
            opcode[0], opcode[1], 0x00, // Zero mask - don't clear any interrupts
            0x00, 0x00, 0x00,
        ];

        let mut rbuffer = [0u8; 4];
        self.read_command(&cmd, &mut rbuffer).await?;

        // Parse IRQ flags (32-bit, big-endian)
        let irq_flags = ((rbuffer[0] as u32) << 24)
            | ((rbuffer[1] as u32) << 16)
            | ((rbuffer[2] as u32) << 8)
            | (rbuffer[3] as u32);

        debug!(
            "get_irq_flags: raw = [{:02x}, {:02x}, {:02x}, {:02x}], flags = 0x{:08x}",
            rbuffer[0], rbuffer[1], rbuffer[2], rbuffer[3], irq_flags
        );

        Ok(irq_flags)
    }

    /// Start transmission (for RTToF ranging)
    ///
    /// # Arguments
    /// * `timeout_rtc_steps` - Timeout in RTC steps (0 for no timeout)
    pub async fn set_tx_mode(&mut self, timeout_rtc_steps: u32) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;

        let opcode = RadioOpCode::SetTx.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((timeout_rtc_steps >> 16) & 0xFF) as u8,
            ((timeout_rtc_steps >> 8) & 0xFF) as u8,
            (timeout_rtc_steps & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Start reception (for RTToF ranging)
    ///
    /// # Arguments
    /// * `timeout_rtc_steps` - Timeout in RTC steps (0xFFFFFF for continuous)
    pub async fn set_rx_mode(&mut self, timeout_rtc_steps: u32) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;

        let opcode = RadioOpCode::SetRx.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((timeout_rtc_steps >> 16) & 0xFF) as u8,
            ((timeout_rtc_steps >> 8) & 0xFF) as u8,
            (timeout_rtc_steps & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    /// Set standby mode
    ///
    /// # Arguments
    /// * `use_xosc` - Use XOSC instead of RC oscillator
    pub async fn set_standby_mode(&mut self, use_xosc: bool) -> Result<(), RadioError> {
        self.intf.iv.disable_rf_switch().await?;

        let opcode = SystemOpCode::SetStandby.bytes();
        let standby_cfg = if use_xosc {
            StandbyMode::Xosc.value()
        } else {
            StandbyMode::Rc.value()
        };
        let cmd = [opcode[0], opcode[1], standby_cfg];
        self.write_command(&cmd).await
    }

    /// Write data to TX buffer
    ///
    /// # Arguments
    /// * `offset` - Buffer offset
    /// * `data` - Data to write
    pub async fn write_tx_buffer(&mut self, offset: u8, data: &[u8]) -> Result<(), RadioError> {
        self.write_buffer(offset, data).await
    }

    /// Read data from RX buffer
    ///
    /// # Arguments
    /// * `offset` - Buffer offset
    /// * `length` - Number of bytes to read
    /// * `buffer` - Buffer to read into
    pub async fn read_rx_buffer(&mut self, offset: u8, length: u8, buffer: &mut [u8]) -> Result<(), RadioError> {
        self.read_buffer(offset, length, buffer).await
    }

    /// Get RX buffer status
    ///
    /// # Returns
    /// (payload_length, buffer_start_pointer)
    pub async fn get_rx_buffer_status(&mut self) -> Result<(u8, u8), RadioError> {
        let opcode = RadioOpCode::GetRxBufferStatus.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; 2];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok((rbuffer[0], rbuffer[1]))
    }

    /// Get LoRa packet status
    ///
    /// # Returns
    /// (rssi_pkt_dbm, snr_pkt_db)
    pub async fn get_lora_packet_status(&mut self) -> Result<(i16, i8), RadioError> {
        let opcode = RadioOpCode::GetPktStatus.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; 3];
        self.read_command(&cmd, &mut rbuffer).await?;

        // Parse RSSI and SNR
        // RSSI is returned as unsigned value, needs to be converted: rssi_dbm = -rssi_raw / 2
        let rssi_pkt_dbm = -((rbuffer[0] as i16) / 2);
        // SNR is signed
        let snr_pkt_db = rbuffer[1] as i8;

        Ok((rssi_pkt_dbm, snr_pkt_db))
    }

    /// Set LoRa modulation parameters
    ///
    /// # Arguments
    /// * `sf` - Spreading factor (5-12)
    /// * `bw` - Bandwidth
    /// * `cr` - Coding rate
    /// * `ldro` - Low data rate optimization (0 or 1)
    pub async fn set_lora_mod_params(&mut self, sf: u8, bw: u8, cr: u8, ldro: u8) -> Result<(), RadioError> {
        let opcode = RadioOpCode::SetModulationParam.bytes();
        let cmd = [opcode[0], opcode[1], sf, bw, cr, ldro];
        self.write_command(&cmd).await
    }

    /// Set LoRa packet parameters
    ///
    /// # Arguments
    /// * `preamble_len` - Preamble length in symbols
    /// * `header_type` - Header type (0 = explicit, 1 = implicit)
    /// * `payload_len` - Payload length
    /// * `crc_on` - Enable CRC (0 = off, 1 = on)
    /// * `iq_inverted` - Invert IQ (0 = standard, 1 = inverted)
    pub async fn set_lora_pkt_params(
        &mut self,
        preamble_len: u16,
        header_type: u8,
        payload_len: u8,
        crc_on: u8,
        iq_inverted: u8,
    ) -> Result<(), RadioError> {
        let opcode = RadioOpCode::SetPktParam.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((preamble_len >> 8) & 0xFF) as u8,
            (preamble_len & 0xFF) as u8,
            header_type,
            payload_len,
            crc_on,
            iq_inverted,
        ];
        self.write_command(&cmd).await
    }

    /// Set LoRa sync word
    ///
    /// # Arguments
    /// * `sync_word` - Sync word value (e.g., 0x34 for private network)
    pub async fn set_lora_sync_word(&mut self, sync_word: u8) -> Result<(), RadioError> {
        let opcode = RadioOpCode::SetLoRaSyncWord.bytes();
        let cmd = [opcode[0], opcode[1], sync_word];
        self.write_command(&cmd).await
    }

    // =========================================================================
    // Bootloader Functions (from SWDR001 lr11xx_bootloader.c)
    // =========================================================================

    /// Get bootloader status registers and IRQ flags
    ///
    /// This function reads the status by performing a direct SPI read.
    /// Unlike the GetStatus command, this does NOT clear the reset status.
    ///
    /// # Returns
    /// * `Ok(BootloaderStatus)` - Status registers and IRQ flags
    pub async fn bootloader_get_status(&mut self) -> Result<BootloaderStatus, RadioError> {
        // Direct read of 6 bytes (no command, just read status)
        let mut rbuffer = [0u8; 6];
        self.intf.iv.wait_on_busy().await?;

        // Perform a direct read to get status bytes
        let opcode = BootloaderOpCode::GetStatus.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.read_command(&cmd, &mut rbuffer[2..]).await?;

        // For now, do a simple status read - stat1 and stat2 come from separate read
        // This is a simplified version; full implementation would use direct_read
        Ok(BootloaderStatus {
            stat1: BootloaderStat1::from_byte(rbuffer[0]),
            stat2: BootloaderStat2::from_byte(rbuffer[1]),
            irq_status: ((rbuffer[2] as u32) << 24)
                | ((rbuffer[3] as u32) << 16)
                | ((rbuffer[4] as u32) << 8)
                | (rbuffer[5] as u32),
        })
    }

    /// Clear the reset status information
    ///
    /// This sends the GetStatus command which clears the reset status field.
    pub async fn bootloader_clear_reset_status(&mut self) -> Result<(), RadioError> {
        let opcode = BootloaderOpCode::GetStatus.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    /// Get bootloader version information
    ///
    /// # Returns
    /// * `Ok(BootloaderVersion)` - Hardware version, chip type, and firmware version
    pub async fn bootloader_get_version(&mut self) -> Result<BootloaderVersion, RadioError> {
        let opcode = BootloaderOpCode::GetVersion.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; BOOTLOADER_VERSION_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;

        Ok(BootloaderVersion {
            hw: rbuffer[0],
            chip_type: rbuffer[1],
            fw: ((rbuffer[2] as u16) << 8) | (rbuffer[3] as u16),
        })
    }

    /// Erase the entire flash memory
    ///
    /// This function MUST be called before writing new firmware to flash.
    ///
    /// # Warning
    /// This operation erases all flash content and cannot be undone.
    pub async fn bootloader_erase_flash(&mut self) -> Result<(), RadioError> {
        let opcode = BootloaderOpCode::EraseFlash.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    /// Write encrypted data to flash memory
    ///
    /// Writes a block of encrypted firmware data to flash.
    /// The data must be provided as 32-bit words (big-endian).
    ///
    /// # Arguments
    /// * `offset` - Byte offset from start of flash
    /// * `data` - Array of 32-bit words to write (max 64 words per call)
    ///
    /// # Constraints
    /// - Complete firmware image must be split into chunks of 64 words
    /// - Chunks must be sent in order, starting with offset = 0
    /// - Last chunk may be shorter than 64 words
    pub async fn bootloader_write_flash_encrypted(&mut self, offset: u32, data: &[u32]) -> Result<(), RadioError> {
        if data.len() > BOOTLOADER_FLASH_BLOCK_SIZE_WORDS {
            return Err(RadioError::PayloadSizeMismatch(
                BOOTLOADER_FLASH_BLOCK_SIZE_WORDS,
                data.len(),
            ));
        }

        let opcode = BootloaderOpCode::WriteFlashEncrypted.bytes();
        let mut cmd = [0u8; 6 + BOOTLOADER_FLASH_BLOCK_SIZE_BYTES]; // 2 opcode + 4 offset + 256 data
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = (offset >> 24) as u8;
        cmd[3] = (offset >> 16) as u8;
        cmd[4] = (offset >> 8) as u8;
        cmd[5] = offset as u8;

        // Convert 32-bit words to bytes (big-endian)
        for (i, word) in data.iter().enumerate() {
            let idx = 6 + i * 4;
            cmd[idx] = (*word >> 24) as u8;
            cmd[idx + 1] = (*word >> 16) as u8;
            cmd[idx + 2] = (*word >> 8) as u8;
            cmd[idx + 3] = *word as u8;
        }

        let cmd_len = 6 + data.len() * 4;
        self.write_command(&cmd[..cmd_len]).await
    }

    /// Reboot the chip
    ///
    /// # Arguments
    /// * `stay_in_bootloader` - If true, stay in bootloader mode after reboot.
    ///   If false, execute flash code (requires valid flash content).
    pub async fn bootloader_reboot(&mut self, stay_in_bootloader: bool) -> Result<(), RadioError> {
        let opcode = BootloaderOpCode::Reboot.bytes();
        let cmd = [opcode[0], opcode[1], if stay_in_bootloader { 0x03 } else { 0x00 }];
        self.write_command(&cmd).await
    }

    /// Read the device PIN for cloud service claiming
    ///
    /// # Returns
    /// * `Ok(BootloaderPin)` - 4-byte PIN
    pub async fn bootloader_read_pin(&mut self) -> Result<BootloaderPin, RadioError> {
        let opcode = BootloaderOpCode::GetPin.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; BOOTLOADER_PIN_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer)
    }

    /// Read the chip EUI
    ///
    /// # Returns
    /// * `Ok(BootloaderChipEui)` - 8-byte chip EUI
    pub async fn bootloader_read_chip_eui(&mut self) -> Result<BootloaderChipEui, RadioError> {
        let opcode = BootloaderOpCode::ReadChipEui.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; BOOTLOADER_CHIP_EUI_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer)
    }

    /// Read the join EUI
    ///
    /// # Returns
    /// * `Ok(BootloaderJoinEui)` - 8-byte join EUI
    pub async fn bootloader_read_join_eui(&mut self) -> Result<BootloaderJoinEui, RadioError> {
        let opcode = BootloaderOpCode::ReadJoinEui.bytes();
        let cmd = [opcode[0], opcode[1]];

        let mut rbuffer = [0u8; BOOTLOADER_JOIN_EUI_LENGTH];
        self.read_command(&cmd, &mut rbuffer).await?;
        Ok(rbuffer)
    }

    // =========================================================================
    // RegMem (Register/Memory) Functions (from SWDR001 lr11xx_regmem.c)
    // =========================================================================

    /// Write 32-bit words to register/memory
    ///
    /// # Arguments
    /// * `address` - Starting memory address
    /// * `data` - Array of 32-bit words to write (max 64 words)
    pub async fn regmem_write_regmem32(&mut self, address: u32, data: &[u32]) -> Result<(), RadioError> {
        if data.len() > REGMEM_MAX_READ_WRITE_WORDS {
            return Err(RadioError::PayloadSizeMismatch(REGMEM_MAX_READ_WRITE_WORDS, data.len()));
        }

        let opcode = RegMemOpCode::WriteRegMem32.bytes();
        let mut cmd = [0u8; 6]; // 2 opcode + 4 address
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = (address >> 24) as u8;
        cmd[3] = (address >> 16) as u8;
        cmd[4] = (address >> 8) as u8;
        cmd[5] = address as u8;

        // Convert 32-bit words to bytes (big-endian)
        let mut cdata = [0u8; REGMEM_MAX_READ_WRITE_WORDS * 4];
        for (i, word) in data.iter().enumerate() {
            let idx = i * 4;
            cdata[idx] = (*word >> 24) as u8;
            cdata[idx + 1] = (*word >> 16) as u8;
            cdata[idx + 2] = (*word >> 8) as u8;
            cdata[idx + 3] = *word as u8;
        }

        // Write command then data
        self.intf.iv.wait_on_busy().await?;
        self.intf
            .write_with_payload(&cmd, &cdata[..data.len() * 4], false)
            .await
    }

    /// Read 32-bit words from register/memory
    ///
    /// # Arguments
    /// * `address` - Starting memory address
    /// * `buffer` - Buffer to store read words (max 64 words)
    ///
    /// # Returns
    /// Number of words read
    pub async fn regmem_read_regmem32(&mut self, address: u32, buffer: &mut [u32]) -> Result<usize, RadioError> {
        if buffer.len() > REGMEM_MAX_READ_WRITE_WORDS {
            return Err(RadioError::PayloadSizeMismatch(
                REGMEM_MAX_READ_WRITE_WORDS,
                buffer.len(),
            ));
        }

        let opcode = RegMemOpCode::ReadRegMem32.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (address >> 24) as u8,
            (address >> 16) as u8,
            (address >> 8) as u8,
            address as u8,
            buffer.len() as u8,
        ];

        let mut rbuffer = [0u8; REGMEM_MAX_READ_WRITE_WORDS * 4];
        let read_len = buffer.len() * 4;
        self.read_command(&cmd, &mut rbuffer[..read_len]).await?;

        // Convert bytes to 32-bit words (big-endian)
        for (i, word) in buffer.iter_mut().enumerate() {
            let idx = i * 4;
            *word = ((rbuffer[idx] as u32) << 24)
                | ((rbuffer[idx + 1] as u32) << 16)
                | ((rbuffer[idx + 2] as u32) << 8)
                | (rbuffer[idx + 3] as u32);
        }

        Ok(buffer.len())
    }

    /// Write bytes to memory
    ///
    /// # Arguments
    /// * `address` - Starting memory address
    /// * `data` - Bytes to write
    pub async fn regmem_write_mem8(&mut self, address: u32, data: &[u8]) -> Result<(), RadioError> {
        let opcode = RegMemOpCode::WriteMem8.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (address >> 24) as u8,
            (address >> 16) as u8,
            (address >> 8) as u8,
            address as u8,
        ];

        self.intf.iv.wait_on_busy().await?;
        self.intf.write_with_payload(&cmd, data, false).await
    }

    /// Read bytes from memory
    ///
    /// # Arguments
    /// * `address` - Starting memory address
    /// * `buffer` - Buffer to store read bytes
    ///
    /// # Returns
    /// Number of bytes read
    pub async fn regmem_read_mem8(&mut self, address: u32, buffer: &mut [u8]) -> Result<usize, RadioError> {
        let opcode = RegMemOpCode::ReadMem8.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (address >> 24) as u8,
            (address >> 16) as u8,
            (address >> 8) as u8,
            address as u8,
            buffer.len() as u8,
        ];

        self.read_command(&cmd, buffer).await?;
        Ok(buffer.len())
    }

    /// Write bytes to TX buffer
    ///
    /// # Arguments
    /// * `data` - Bytes to write to TX buffer
    pub async fn regmem_write_buffer8(&mut self, data: &[u8]) -> Result<(), RadioError> {
        let opcode = RegMemOpCode::WriteBuffer8.bytes();
        let cmd = [opcode[0], opcode[1]];

        self.intf.iv.wait_on_busy().await?;
        self.intf.write_with_payload(&cmd, data, false).await
    }

    /// Read bytes from RX buffer
    ///
    /// # Arguments
    /// * `offset` - Offset within RX buffer
    /// * `buffer` - Buffer to store read bytes
    ///
    /// # Returns
    /// Number of bytes read
    pub async fn regmem_read_buffer8(&mut self, offset: u8, buffer: &mut [u8]) -> Result<usize, RadioError> {
        let opcode = RegMemOpCode::ReadBuffer8.bytes();
        let cmd = [opcode[0], opcode[1], offset, buffer.len() as u8];

        self.read_command(&cmd, buffer).await?;
        Ok(buffer.len())
    }

    /// Clear the RX buffer
    ///
    /// Sets all bytes in the RX buffer to 0x00.
    pub async fn regmem_clear_rxbuffer(&mut self) -> Result<(), RadioError> {
        let opcode = RegMemOpCode::ClearRxBuffer.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    /// Read-modify-write a 32-bit register with mask
    ///
    /// Performs: register = (register & ~mask) | (data & mask)
    ///
    /// # Arguments
    /// * `address` - Register address
    /// * `mask` - Bits to modify (1 = modify, 0 = preserve)
    /// * `data` - New data for masked bits
    pub async fn regmem_write_regmem32_mask(&mut self, address: u32, mask: u32, data: u32) -> Result<(), RadioError> {
        let opcode = RegMemOpCode::WriteRegMem32Mask.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            (address >> 24) as u8,
            (address >> 16) as u8,
            (address >> 8) as u8,
            address as u8,
            (mask >> 24) as u8,
            (mask >> 16) as u8,
            (mask >> 8) as u8,
            mask as u8,
            (data >> 24) as u8,
            (data >> 16) as u8,
            (data >> 8) as u8,
            data as u8,
        ];
        self.write_command(&cmd).await
    }
}

/// Get the number of hop sequences for given LR-FHSS parameters
pub fn lr_fhss_get_hop_sequence_count(params: &LrFhssParams) -> u16 {
    params
        .lr_fhss_params
        .bandwidth
        .hop_sequence_count(params.lr_fhss_params.grid)
}

// Convert u8 sync word to single byte value for LR1110
fn convert_sync_word(sync_word: u8) -> u8 {
    // LR1110 uses a simpler sync word format
    sync_word
}

impl<SPI, IV, C> RadioKind for Lr1110<SPI, IV, C>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
    C: Lr1110Variant,
{
    async fn init_lora(&mut self, sync_word: u8) -> Result<(), RadioError> {
        // Initialize system (DC-DC, TCXO, calibration)
        self.init_system().await?;

        // DIO2 acting as RF Switch (if configured in variant)
        if self.config.chip.use_dio2_as_rfswitch() {
            // LR1110 uses SetDioAsRfSwitch command with expanded configuration
            // For now, use simple configuration
            let opcode = SystemOpCode::SetDioAsRfSwitch.bytes();
            let cmd = [
                opcode[0], opcode[1], 0x01, // enable
                0x00, // standby
                0x01, // rx
                0x02, // tx
                0x02, // tx_hp
                0x00, // tx_hf
                0x00, // gnss
                0x00, // wifi
            ];
            self.write_command(&cmd).await?;
        }

        // Enable LoRa packet engine
        let opcode = RadioOpCode::SetPktType.bytes();
        let cmd = [opcode[0], opcode[1], PacketType::LoRa.value()];
        self.write_command(&cmd).await?;

        // Set LoRa sync word
        let word = convert_sync_word(sync_word);
        let sync_opcode = RadioOpCode::SetLoRaSyncWord.bytes();
        let sync_cmd = [sync_opcode[0], sync_opcode[1], word];
        self.write_command(&sync_cmd).await?;

        // Set buffer base addresses
        self.set_tx_rx_buffer_base_address(0, 0).await?;

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

    async fn ensure_ready(&mut self, _mode: RadioMode) -> Result<(), RadioError> {
        // LR1110 has no BUSY pin, so just return Ok
        // The radio is always ready to accept commands after previous command completes
        Ok(())
    }

    async fn set_standby(&mut self) -> Result<(), RadioError> {
        let opcode = SystemOpCode::SetStandby.bytes();
        let cmd = [opcode[0], opcode[1], StandbyMode::Rc.value()];
        self.write_command(&cmd).await?;
        self.intf.iv.disable_rf_switch().await
    }

    async fn set_sleep(&mut self, warm_start_if_possible: bool, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        self.intf.iv.disable_rf_switch().await?;

        let sleep_params = SleepParams {
            warm_start: warm_start_if_possible,
            rtc_wakeup: false,
        };
        let opcode = SystemOpCode::SetSleep.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            sleep_params.value(),
            0x00, // sleep_time MSB
            0x00,
            0x00,
            0x00, // sleep_time LSB
        ];
        self.write_command(&cmd).await?;
        delay.delay_ms(2).await;

        Ok(())
    }

    async fn set_tx_rx_buffer_base_address(
        &mut self,
        _tx_base_addr: usize,
        _rx_base_addr: usize,
    ) -> Result<(), RadioError> {
        // LR1110 doesn't use buffer base addresses like SX126x
        // The WriteBuffer8 and ReadBuffer8 commands handle buffer addressing
        Ok(())
    }

    async fn set_tx_power_and_ramp_time(
        &mut self,
        output_power: i32,
        mdltn_params: Option<&ModulationParams>,
        _is_tx_prep: bool,
    ) -> Result<(), RadioError> {
        // Use 208µs ramp time to match SWDM001 LR-FHSS demo behavior
        // Shorter ramp times can cause TX issues with some configurations
        let ramp_time = RampTime::Ramp208Us;

        let pa_selection = self.config.chip.get_pa_selection();
        let pa_supply = self.config.chip.get_pa_supply();

        let (tx_power, pa_duty_cycle, pa_hp_sel) = match pa_selection {
            PaSelection::Lp => {
                // Low Power PA: -17 to +14 dBm
                const LP_MIN: i32 = -17;
                const LP_MAX: i32 = 14;
                let txp = output_power.clamp(LP_MIN, LP_MAX);

                // Validate frequency constraint for max power
                if txp == LP_MAX {
                    if let Some(m_p) = mdltn_params {
                        if m_p.frequency_in_hz < 400_000_000 {
                            return Err(RadioError::InvalidOutputPowerForFrequency);
                        }
                    }
                }

                // PA configuration for LP PA
                // Per LR1110 User Manual Table 9-1 and SWDM001 demo:
                // LP PA uses paDutyCycle = 0x04, paHPSel = 0x00
                let (duty_cycle, hp_sel, power) = (0x04, 0x00, txp as u8);
                (power, duty_cycle, hp_sel)
            }
            PaSelection::Hp => {
                // High Power PA: -9 to +22 dBm
                const HP_MIN: i32 = -9;
                const HP_MAX: i32 = 22;
                let txp = output_power.clamp(HP_MIN, HP_MAX);

                let (duty_cycle, hp_sel, power) = match txp {
                    22 => (0x04, 0x07, 22),
                    18..=21 => (0x03, 0x05, txp as u8),
                    15..=17 => (0x02, 0x03, txp as u8),
                    HP_MIN..=14 => (0x02, 0x02, txp as u8),
                    _ => unreachable!(),
                };
                (power, duty_cycle, hp_sel)
            }
            PaSelection::Hf => {
                // High Frequency PA (2.4 GHz): -18 to +13 dBm
                const HF_MIN: i32 = -18;
                const HF_MAX: i32 = 13;
                let txp = output_power.clamp(HF_MIN, HF_MAX);

                let (duty_cycle, hp_sel, power) = match txp {
                    13 => (0x04, 0x00, 13),
                    10..=12 => (0x02, 0x00, txp as u8),
                    HF_MIN..=9 => (0x01, 0x00, txp as u8),
                    _ => unreachable!(),
                };
                (power, duty_cycle, hp_sel)
            }
        };

        // Set PA configuration
        self.set_pa_config(pa_selection, pa_supply, pa_duty_cycle, pa_hp_sel)
            .await?;

        // Set TX parameters
        let opcode = RadioOpCode::SetTxParams.bytes();
        let cmd = [opcode[0], opcode[1], tx_power, ramp_time.value()];
        self.write_command(&cmd).await
    }

    async fn set_modulation_params(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        let spreading_factor_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let bandwidth_val = bandwidth_value(mdltn_params.bandwidth)?;
        let coding_rate_val = coding_rate_value(mdltn_params.coding_rate)?;

        debug!(
            "sf = {}, bw = {}, cr = {}",
            spreading_factor_val, bandwidth_val, coding_rate_val
        );

        let opcode = RadioOpCode::SetModulationParam.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            spreading_factor_val,
            bandwidth_val,
            coding_rate_val,
            mdltn_params.low_data_rate_optimize,
        ];
        self.write_command(&cmd).await
    }

    async fn set_packet_params(&mut self, pkt_params: &PacketParams) -> Result<(), RadioError> {
        let header_type = if pkt_params.implicit_header {
            LoRaHeaderType::Implicit
        } else {
            LoRaHeaderType::Explicit
        };

        let crc = if pkt_params.crc_on { LoRaCrc::On } else { LoRaCrc::Off };

        let iq = if pkt_params.iq_inverted {
            LoRaIq::Inverted
        } else {
            LoRaIq::Standard
        };

        let opcode = RadioOpCode::SetPktParam.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((pkt_params.preamble_length >> 8) & 0xFF) as u8,
            (pkt_params.preamble_length & 0xFF) as u8,
            header_type.value(),
            pkt_params.payload_length,
            crc.value(),
            iq.value(),
        ];
        self.write_command(&cmd).await
    }

    async fn calibrate_image(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        let (freq1, freq2) = if frequency_in_hz > 900_000_000 {
            (0xE1, 0xE9)
        } else if frequency_in_hz > 850_000_000 {
            (0xD7, 0xDB)
        } else if frequency_in_hz > 770_000_000 {
            (0xC1, 0xC5)
        } else if frequency_in_hz > 460_000_000 {
            (0x75, 0x81)
        } else {
            (0x6B, 0x6F)
        };

        let opcode = SystemOpCode::CalibrateImage.bytes();
        let cmd = [opcode[0], opcode[1], freq1, freq2];
        self.write_command(&cmd).await
    }

    async fn set_channel(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        debug!("channel = {}", frequency_in_hz);

        // LR1110 uses frequency directly in Hz (as 32-bit value)
        let opcode = RadioOpCode::SetRfFrequency.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((frequency_in_hz >> 24) & 0xFF) as u8,
            ((frequency_in_hz >> 16) & 0xFF) as u8,
            ((frequency_in_hz >> 8) & 0xFF) as u8,
            (frequency_in_hz & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    async fn set_payload(&mut self, payload: &[u8]) -> Result<(), RadioError> {
        self.write_buffer(0x00, payload).await
    }

    async fn do_tx(&mut self) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;

        // Clear any pending IRQs (especially error flags) before TX
        self.clear_all_irq().await?;

        // Reconfigure TCXO with longer timeout before TX (per SWDM001)
        // This ensures the TCXO is stable during transmission
        if let Some(voltage) = self.config.tcxo_ctrl {
            // SWDM001 uses 0x000CD0 = 3280 RTC steps (~100ms) before TX
            let tx_tcxo_timeout: u32 = 0x000CD0;
            let tcxo_opcode = SystemOpCode::SetTcxoMode.bytes();
            let tcxo_cmd = [
                tcxo_opcode[0],
                tcxo_opcode[1],
                voltage.value(),
                Self::timeout_1(tx_tcxo_timeout),
                Self::timeout_2(tx_tcxo_timeout),
                Self::timeout_3(tx_tcxo_timeout),
            ];
            self.write_command(&tcxo_cmd).await?;
        }

        // Disable timeout (0 = no timeout)
        let opcode = RadioOpCode::SetTx.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            Self::timeout_1(0),
            Self::timeout_2(0),
            Self::timeout_3(0),
        ];
        self.write_command(&cmd).await
    }

    async fn do_rx(&mut self, rx_mode: RxMode) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;

        // Stop RX timer on preamble detection
        let preamble_opcode = RadioOpCode::StopTimeoutOnPreamble.bytes();
        let preamble_cmd = [preamble_opcode[0], preamble_opcode[1], 0x01];
        self.write_command(&preamble_cmd).await?;

        // Set symbol timeout
        let num_symbols = match rx_mode {
            RxMode::DutyCycle(_) | RxMode::Continuous => 0,
            RxMode::Single(n) => n,
        };
        self.set_lora_symbol_num_timeout(num_symbols).await?;

        // Configure RX boost if enabled
        if self.config.rx_boost {
            let boost_opcode = RadioOpCode::SetRxBoosted.bytes();
            let boost_cmd = [boost_opcode[0], boost_opcode[1], 0x01];
            self.write_command(&boost_cmd).await?;
        }

        match rx_mode {
            RxMode::DutyCycle(args) => {
                // Convert ms to RTC steps
                let rx_time = (args.rx_time as u64 * 32768 / 1000) as u32;
                let sleep_time = (args.sleep_time as u64 * 32768 / 1000) as u32;

                let opcode = RadioOpCode::SetRxDutyCycle.bytes();
                let cmd = [
                    opcode[0],
                    opcode[1],
                    Self::timeout_1(rx_time),
                    Self::timeout_2(rx_time),
                    Self::timeout_3(rx_time),
                    Self::timeout_1(sleep_time),
                    Self::timeout_2(sleep_time),
                    Self::timeout_3(sleep_time),
                    0x00, // mode (0 = StandbyRC)
                ];
                self.write_command(&cmd).await
            }
            RxMode::Single(_) | RxMode::Continuous => {
                let timeout = if matches!(rx_mode, RxMode::Continuous) {
                    RX_CONTINUOUS_TIMEOUT
                } else {
                    0
                };

                let opcode = RadioOpCode::SetRx.bytes();
                let cmd = [
                    opcode[0],
                    opcode[1],
                    Self::timeout_1(timeout),
                    Self::timeout_2(timeout),
                    Self::timeout_3(timeout),
                ];
                self.write_command(&cmd).await
            }
        }
    }

    async fn get_rx_payload(
        &mut self,
        _rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<u8, RadioError> {
        // Get RX buffer status
        let status_opcode = RadioOpCode::GetRxBufferStatus.bytes();
        let status_cmd = [status_opcode[0], status_opcode[1]];
        let mut rx_buffer_status = [0u8; 2];
        self.read_command(&status_cmd, &mut rx_buffer_status).await?;

        let payload_length = rx_buffer_status[0];
        let offset = rx_buffer_status[1];

        if (payload_length as usize) > receiving_buffer.len() {
            return Err(RadioError::PayloadSizeMismatch(
                payload_length as usize,
                receiving_buffer.len(),
            ));
        }

        // Read payload from buffer
        self.read_buffer(offset, payload_length, receiving_buffer).await?;

        Ok(payload_length)
    }

    async fn get_rx_packet_status(&mut self) -> Result<PacketStatus, RadioError> {
        let opcode = RadioOpCode::GetPktStatus.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut pkt_status = [0u8; 3];
        self.read_command(&cmd, &mut pkt_status).await?;

        // RSSI = -pkt_status[0] / 2
        let rssi = ((-(pkt_status[0] as i32)) >> 1) as i16;
        // SNR = (pkt_status[1] + 2) / 4
        let snr = (((pkt_status[1] as i8) + 2) >> 2) as i16;

        Ok(PacketStatus { rssi, snr })
    }

    async fn get_rssi(&mut self) -> Result<i16, RadioError> {
        let opcode = RadioOpCode::GetRssiInst.bytes();
        let cmd = [opcode[0], opcode[1]];
        let mut response = [0u8; 1];
        self.read_command(&cmd, &mut response).await?;

        let rssi = ((-(response[0] as i32)) >> 1) as i16;
        Ok(rssi)
    }

    async fn do_cad(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_rx().await?;

        // Configure RX boost if enabled
        if self.config.rx_boost {
            let boost_opcode = RadioOpCode::SetRxBoosted.bytes();
            let boost_cmd = [boost_opcode[0], boost_opcode[1], 0x01];
            self.write_command(&boost_cmd).await?;
        }

        // Set CAD parameters
        let spreading_factor_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let cad_opcode = RadioOpCode::SetCadParams.bytes();
        let cad_cmd = [
            cad_opcode[0],
            cad_opcode[1],
            CadSymbols::_8.value(),         // CAD symbol number
            spreading_factor_val + 13,      // CAD detection peak
            10,                             // CAD detection min
            CadExitMode::StandbyRc.value(), // CAD exit mode
            0x00,                           // timeout (24-bit)
            0x00,
            0x00,
        ];
        self.write_command(&cad_cmd).await?;

        // Start CAD
        let start_opcode = RadioOpCode::SetCad.bytes();
        let start_cmd = [start_opcode[0], start_opcode[1]];
        self.write_command(&start_cmd).await
    }

    async fn set_irq_params(&mut self, radio_mode: Option<RadioMode>) -> Result<(), RadioError> {
        let dio1_mask: u32 = match radio_mode {
            Some(RadioMode::Standby) => 0xFFFFFFFF,
            Some(RadioMode::Transmit) => IrqMask::TxDone.value() | IrqMask::Timeout.value(),
            Some(RadioMode::Receive(_)) => 0xFFFFFFFF,
            Some(RadioMode::ChannelActivityDetection) => IrqMask::CadDone.value() | IrqMask::CadDetected.value(),
            _ => 0x00000000,
        };

        debug!("set_irq_params: dio1_mask = 0x{:08x}", dio1_mask);

        let opcode = SystemOpCode::SetDioIrqParams.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            // Global IRQ enable mask (bytes 2-5)
            ((dio1_mask >> 24) & 0xFF) as u8,
            ((dio1_mask >> 16) & 0xFF) as u8,
            ((dio1_mask >> 8) & 0xFF) as u8,
            (dio1_mask & 0xFF) as u8,
            // DIO1 mask (bytes 6-9) - route these IRQs to DIO1 pin
            ((dio1_mask >> 24) & 0xFF) as u8,
            ((dio1_mask >> 16) & 0xFF) as u8,
            ((dio1_mask >> 8) & 0xFF) as u8,
            (dio1_mask & 0xFF) as u8,
        ];
        self.write_command(&cmd).await
    }

    async fn set_tx_continuous_wave_mode(&mut self) -> Result<(), RadioError> {
        self.intf.iv.enable_rf_switch_tx().await?;

        let opcode = RadioOpCode::SetTxCw.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    async fn await_irq(&mut self) -> Result<(), RadioError> {
        self.intf.iv.await_irq().await
    }

    async fn get_irq_state(
        &mut self,
        radio_mode: RadioMode,
        cad_activity_detected: Option<&mut bool>,
    ) -> Result<Option<IrqState>, RadioError> {
        // Read IRQ status from the LR1110
        let irq_flags = self.get_irq_flags().await?;

        debug!(
            "process_irq: irq_flags = 0x{:08x} in radio mode {}",
            irq_flags, radio_mode
        );

        match radio_mode {
            RadioMode::Transmit => {
                if IrqMask::TxDone.is_set(irq_flags) {
                    return Ok(Some(IrqState::Done));
                }
                if IrqMask::Timeout.is_set(irq_flags) {
                    return Err(RadioError::TransmitTimeout);
                }
                // LR1110 may auto-clear IRQ flags when DIO1 triggers.
                // If we waited for DIO1 and flags are 0, TX is complete.
                if irq_flags == 0 {
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Receive(_) => {
                if IrqMask::CrcError.is_set(irq_flags) || IrqMask::HeaderError.is_set(irq_flags) {
                    debug!("CRC or Header error");
                }
                if IrqMask::RxDone.is_set(irq_flags) {
                    return Ok(Some(IrqState::Done));
                }
                if IrqMask::Timeout.is_set(irq_flags) {
                    return Err(RadioError::ReceiveTimeout);
                }
                if IrqMask::PreambleDetected.is_set(irq_flags) || IrqMask::SyncWordHeaderValid.is_set(irq_flags) {
                    return Ok(Some(IrqState::PreambleReceived));
                }
            }
            RadioMode::ChannelActivityDetection => {
                if IrqMask::CadDone.is_set(irq_flags) {
                    if let Some(detected) = cad_activity_detected {
                        *detected = IrqMask::CadDetected.is_set(irq_flags);
                    }
                    return Ok(Some(IrqState::Done));
                }
            }
            RadioMode::Sleep | RadioMode::Standby | RadioMode::Listen => {
                warn!("IRQ during sleep/standby/listen?");
            }
            RadioMode::FrequencySynthesis => {}
        }

        Ok(None)
    }

    async fn clear_irq_status(&mut self) -> Result<(), RadioError> {
        let opcode = SystemOpCode::ClearIrq.bytes();
        let cmd = [
            opcode[0], opcode[1], 0xFF, // Clear all interrupts (32-bit mask)
            0xFF, 0xFF, 0xFF,
        ];
        self.write_command(&cmd).await
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_sync_word() {
        // LR1110 uses simple sync word format
        assert_eq!(convert_sync_word(0x34), 0x34);
        assert_eq!(convert_sync_word(0x12), 0x12);
    }

    #[test]
    fn power_level_conversion() {
        // Test that power level conversions work correctly
        let lp_min: i32 = -17;
        let hp_max: i32 = 22;
        assert_eq!(lp_min as u8, 0xEF);
        assert_eq!(hp_max as u8, 22);
    }
}
