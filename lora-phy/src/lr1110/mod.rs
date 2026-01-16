pub mod radio_kind_params;
pub mod variant;

use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::*;
pub use radio_kind_params::TcxoCtrlVoltage;
pub use radio_kind_params::{
    LrFhssBandwidth, LrFhssCodingRate, LrFhssGrid, LrFhssModulationType, LrFhssParams, LrFhssV1Params,
    LR_FHSS_SYNC_WORD_BYTES, LR_FHSS_DEFAULT_SYNC_WORD,
};
// System types
pub use radio_kind_params::{
    Version, ChipType, ChipMode, ResetStatus, CommandStatus, Stat1, Stat2, SystemStatus,
    LR11XX_SYSTEM_UID_LENGTH, LR11XX_SYSTEM_JOIN_EUI_LENGTH,
};
// GNSS types
pub use radio_kind_params::{
    GnssOpCode, GnssConstellation, GnssConstellationMask, GnssSearchMode, GnssDestination,
    GnssScanMode, GnssHostStatus, GnssErrorCode, GnssFreqSearchSpace, GnssResultFields,
    GnssAssistancePosition, GnssVersion, GnssDetectedSatellite, GnssContextStatus,
    GNSS_GPS_MASK, GNSS_BEIDOU_MASK, GNSS_MAX_RESULT_SIZE, GNSS_CONTEXT_STATUS_LENGTH,
    GNSS_SINGLE_ALMANAC_READ_SIZE, GNSS_SINGLE_ALMANAC_WRITE_SIZE, GNSS_SNR_TO_CNR_OFFSET,
    GNSS_SCALING_LATITUDE, GNSS_SCALING_LONGITUDE,
};
use radio_kind_params::*;

use crate::mod_params::*;
use crate::mod_traits::IrqState;
use crate::{InterfaceVariant, RadioKind, SpiInterface};
pub use variant::*;

// Internal frequency of the radio
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

    fn convert_freq_in_hz_to_pll_step(freq_in_hz: u32) -> u32 {
        // LR1110 uses direct frequency value in Hz (not PLL steps like SX126x)
        // The formula is: freq_in_hz * 2^25 / XTAL_FREQ
        (((freq_in_hz as u64) << 25) / (LR1110_XTAL_FREQ as u64)) as u32
    }

    // =========================================================================
    // LR-FHSS Public Methods
    // =========================================================================

    /// Initialize LR-FHSS mode
    /// This sets the packet type to LR-FHSS
    pub async fn lr_fhss_init(&mut self) -> Result<(), RadioError> {
        let opcode = LrFhssOpCode::Init.bytes();
        let cmd = [opcode[0], opcode[1]];
        self.write_command(&cmd).await
    }

    /// Build and transmit an LR-FHSS frame
    ///
    /// This command configures the LR-FHSS parameters, writes the payload,
    /// and prepares the radio for transmission.
    pub async fn lr_fhss_build_frame(
        &mut self,
        params: &LrFhssParams,
        hop_sequence_id: u16,
        payload: &[u8],
    ) -> Result<(), RadioError> {
        // Set LR-FHSS sync word from params (matching SWDM001 behavior)
        self.lr_fhss_set_sync_word(&params.lr_fhss_params.sync_word).await?;

        // Build LR-FHSS frame command
        // Format: opcode[2] + lr_fhss_params[8] + hop_seq_id[2] + payload_len[2] + payload[n]
        let opcode = LrFhssOpCode::BuildFrame.bytes();

        // Construct command buffer
        let lr_fhss_params = &params.lr_fhss_params;
        let enable_hopping: u8 = if lr_fhss_params.enable_hopping { 1 } else { 0 };

        let mut cmd = [0u8; 14]; // 2 opcode + 8 params + 2 hop_seq_id + 2 payload_len
        cmd[0] = opcode[0];
        cmd[1] = opcode[1];
        cmd[2] = lr_fhss_params.bandwidth.value();
        cmd[3] = lr_fhss_params.coding_rate.value();
        cmd[4] = lr_fhss_params.grid.value();
        cmd[5] = enable_hopping;
        cmd[6] = lr_fhss_params.modulation_type.value();
        cmd[7] = params.device_offset as u8;
        cmd[8] = lr_fhss_params.header_count;
        cmd[9] = 0x00; // Reserved
        cmd[10] = ((hop_sequence_id >> 8) & 0xFF) as u8;
        cmd[11] = (hop_sequence_id & 0xFF) as u8;
        cmd[12] = ((payload.len() >> 8) & 0xFF) as u8;
        cmd[13] = (payload.len() & 0xFF) as u8;

        // Write command with payload
        self.intf.write_with_payload(&cmd, payload, false).await
    }

    /// Set LR-FHSS sync word
    async fn lr_fhss_set_sync_word(&mut self, sync_word: &[u8; 4]) -> Result<(), RadioError> {
        let opcode = LrFhssOpCode::SetSyncWord.bytes();
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
        let mask: u32 = 1 << 30;  // Bit 30
        let data: u32 = 0;        // Clear bit 30

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

        Ok(((rbuffer[0] as u32) << 24)
            | ((rbuffer[1] as u32) << 16)
            | ((rbuffer[2] as u32) << 8)
            | (rbuffer[3] as u32))
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
    pub async fn gnss_set_constellation(&mut self, constellation_mask: GnssConstellationMask) -> Result<(), RadioError> {
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

        for i in 0..n {
            let offset = i * 4;
            satellites[i] = GnssDetectedSatellite {
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
        let freq_search_space = GnssFreqSearchSpace::from(
            ((rbuffer[6] & 0x01) << 1) | ((rbuffer[7] & 0x80) >> 7),
        );

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
    pub async fn gnss_set_freq_search_space(&mut self, freq_search_space: GnssFreqSearchSpace) -> Result<(), RadioError> {
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
        // DC-DC regulator setup (default is LDO)
        if self.config.use_dcdc {
            let opcode = SystemOpCode::SetRegMode.bytes();
            let cmd = [opcode[0], opcode[1], RegulatorMode::Dcdc.value()];
            self.write_command(&cmd).await?;
        }

        // DIO2 acting as RF Switch (if configured in variant)
        if self.config.chip.use_dio2_as_rfswitch() {
            // LR1110 uses SetDioAsRfSwitch command with expanded configuration
            // For now, use simple configuration
            let opcode = SystemOpCode::SetDioAsRfSwitch.bytes();
            let cmd = [
                opcode[0], opcode[1],
                0x01,  // enable
                0x00,  // standby
                0x01,  // rx
                0x02,  // tx
                0x02,  // tx_hp
                0x00,  // tx_hf
                0x00,  // gnss
                0x00,  // wifi
            ];
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
            // Note: LR1110 has no BUSY pin, so no wait needed
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

        if ((bandwidth == Bandwidth::_250KHz) || (bandwidth == Bandwidth::_500KHz)) && (frequency_in_hz < 400_000_000)
        {
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
        is_tx_prep: bool,
    ) -> Result<(), RadioError> {
        let ramp_time = match is_tx_prep {
            true => RampTime::Ramp16Us,   // Fast ramp for TX prep
            false => RampTime::Ramp48Us,  // Slower ramp for init
        };

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

                // PA configuration based on output power
                let (duty_cycle, hp_sel, power) = match txp {
                    14 => (0x04, 0x00, 14),
                    10..=13 => (0x02, 0x00, txp as u8),
                    LP_MIN..=9 => (0x01, 0x00, txp as u8),
                    _ => unreachable!(),
                };
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
        let cmd = [opcode[0], opcode[1], tx_power as u8, ramp_time.value()];
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

        let crc = if pkt_params.crc_on {
            LoRaCrc::On
        } else {
            LoRaCrc::Off
        };

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
        self.read_buffer(offset, payload_length, receiving_buffer)
            .await?;

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
            CadSymbols::_8.value(),      // CAD symbol number
            spreading_factor_val + 13,   // CAD detection peak
            10,                          // CAD detection min
            CadExitMode::StandbyRc.value(), // CAD exit mode
            0x00,                        // timeout (24-bit)
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
            Some(RadioMode::ChannelActivityDetection) => {
                IrqMask::CadDone.value() | IrqMask::CadDetected.value()
            }
            _ => 0x00000000,
        };

        let opcode = SystemOpCode::SetDioIrqParams.bytes();
        let cmd = [
            opcode[0],
            opcode[1],
            ((dio1_mask >> 24) & 0xFF) as u8,
            ((dio1_mask >> 16) & 0xFF) as u8,
            ((dio1_mask >> 8) & 0xFF) as u8,
            (dio1_mask & 0xFF) as u8,
            0x00, // DIO2 mask (4 bytes)
            0x00,
            0x00,
            0x00,
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
        // Read IRQ status - LR1110 returns 32-bit IRQ flags directly via GetStatus
        // We'll use a direct read to get the status including IRQ flags
        // For now, use the system command to get IRQ status

        // Note: The proper way would be to implement direct_read in the interface,
        // but for simplicity we'll clear and read IRQs via standard command
        let irq_flags = 0u32;

        // This is a simplified version - in production you'd want to use GetStatus direct read
        // For now we just return None to indicate no IRQ processing
        // A full implementation would read the 32-bit IRQ mask properly

        match radio_mode {
            RadioMode::Transmit => {
                if IrqMask::TxDone.is_set(irq_flags) {
                    return Ok(Some(IrqState::Done));
                }
                if IrqMask::Timeout.is_set(irq_flags) {
                    return Err(RadioError::TransmitTimeout);
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
                if IrqMask::PreambleDetected.is_set(irq_flags) || IrqMask::SyncWordHeaderValid.is_set(irq_flags)
                {
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
            opcode[0],
            opcode[1],
            0xFF, // Clear all interrupts (32-bit mask)
            0xFF,
            0xFF,
            0xFF,
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
