//! LR1110 Firmware Update Example
//!
//! This example demonstrates how to update the firmware on the LR1110 radio chip
//! using the bootloader interface. It implements the same process as Semtech's
//! SWTL001 firmware updater tool.
//!
//! ## Firmware Update Process
//!
//! 1. Reset the chip and enter bootloader mode
//! 2. Read and validate bootloader version
//! 3. Read device identifiers (PIN, Chip EUI, Join EUI)
//! 4. Erase flash memory
//! 5. Write encrypted firmware image in 256-byte chunks
//! 6. Reboot and verify new firmware version
//!
//! ## Obtaining Firmware Images
//!
//! Firmware images are available from Semtech and are pre-encrypted. You need to:
//!
//! 1. Download the SWTL001 package from Semtech
//! 2. Extract the firmware header file (e.g., `lr1110_transceiver_0401.h`)
//! 3. Convert the C array to Rust format (see `firmware_image` module below)
//!
//! ## Hardware Connections
//!
//! Same as other LR1110 examples:
//! - SPI2_SCK:  PB10
//! - SPI2_MISO: PA9
//! - SPI2_MOSI: PC3
//! - SPI2_NSS:  PD14
//! - LR1110_RESET: PB2
//! - LR1110_BUSY:  PB13
//! - LR1110_DIO1:  PB14
//!
//! ## Usage
//!
//! ```bash
//! cargo run --release --bin lr1110_firmware_update
//! ```
//!
//! ## Reference
//!
//! - SWTL001: LR11xx Firmware Update Tool
//! - AN1200.57: LR1110 Upgrade of the Program Memory

#![no_std]
#![no_main]

#[path = "../iv.rs"]
mod iv;

#[path = "../lr1110_firmware_0401.rs"]
mod firmware;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::rcc::{
    AHB5Prescaler, AHBPrescaler, APBPrescaler, PllDiv, PllMul, PllPreDiv, PllSource, Sysclk, VoltageScale,
};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_stm32::{Config, bind_interrupts};
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::lr1110::variant::Lr1110 as Lr1110Chip;
use lora_phy::lr1110::{self as lr1110_module, TcxoCtrlVoltage};
use lora_phy::lr1110::{BOOTLOADER_FLASH_BLOCK_SIZE_WORDS, BootloaderVersion, Version};
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

// Bind EXTI interrupts for PB13 (BUSY) and PB14 (DIO1)
bind_interrupts!(struct Irqs {
    EXTI13 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI13>;
    EXTI14 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI14>;
});

// ============================================================================
// Firmware Image Configuration
// ============================================================================

/// Firmware update target type
#[derive(Clone, Copy, PartialEq, Debug, defmt::Format)]
#[allow(dead_code)]
pub enum FirmwareType {
    /// LR1110 Transceiver firmware
    Lr1110Transceiver,
    /// LR1110 Modem V1 firmware
    Lr1110ModemV1,
    /// LR1120 Transceiver firmware
    Lr1120Transceiver,
    /// LR1121 Transceiver firmware
    Lr1121Transceiver,
    /// LR1121 Modem V2 firmware
    Lr1121ModemV2,
}

/// Expected bootloader version for each chip type
impl FirmwareType {
    fn expected_bootloader_version(&self) -> u16 {
        match self {
            FirmwareType::Lr1110Transceiver | FirmwareType::Lr1110ModemV1 => 0x6500,
            FirmwareType::Lr1120Transceiver => 0x2000,
            FirmwareType::Lr1121Transceiver | FirmwareType::Lr1121ModemV2 => 0x2100,
        }
    }
}

/// Firmware update status
#[derive(Clone, Copy, PartialEq, Debug, defmt::Format)]
pub enum FirmwareUpdateStatus {
    /// Update completed successfully
    Ok,
    /// Wrong chip type or bootloader version
    WrongChipType,
    /// Firmware verification failed after update
    VerificationFailed,
    /// Flash erase failed
    EraseFailed,
    /// Flash write failed
    WriteFailed,
}

// ============================================================================
// Firmware Image Configuration
// ============================================================================
//
// The firmware image is loaded from lr1110_firmware_0401.rs which contains
// the LR1110 Transceiver firmware v0401 from Semtech's radio_firmware_images
// repository: https://github.com/Lora-net/radio_firmware_images
//
// To use a different firmware version:
// 1. Download the header from Semtech's repository
// 2. Convert the C array to Rust format
// 3. Create a new module file and update the path above
//
// ============================================================================

/// Target firmware type for this update
const FIRMWARE_TYPE: FirmwareType = FirmwareType::Lr1110Transceiver;

/// Expected firmware version after update (from firmware module)
const EXPECTED_FIRMWARE_VERSION: u16 = firmware::LR11XX_FIRMWARE_VERSION;

/// Firmware image data (from firmware module)
const FIRMWARE_IMAGE: &[u32] = firmware::LR11XX_FIRMWARE_IMAGE;

// ============================================================================
// Main Application
// ============================================================================

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize STM32WBA65RI peripherals
    let mut config = Config::default();

    // Configure PLL1 for 96 MHz system clock
    config.rcc.pll1 = Some(embassy_stm32::rcc::Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL30,
        divr: Some(PllDiv::DIV5),
        divq: None,
        divp: Some(PllDiv::DIV30),
        frac: Some(0),
    });

    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV1;
    config.rcc.apb2_pre = APBPrescaler::DIV1;
    config.rcc.apb7_pre = APBPrescaler::DIV1;
    config.rcc.ahb5_pre = AHB5Prescaler::DIV4;
    config.rcc.voltage_scale = VoltageScale::RANGE1;
    config.rcc.sys = Sysclk::PLL1_R;

    let p = embassy_stm32::init(config);

    info!("==============================================");
    info!("LR1110 Firmware Update Example");
    info!("==============================================");

    // Check if placeholder firmware is being used
    if FIRMWARE_IMAGE.is_empty() {
        error!("ERROR: No firmware image provided!");
        error!("Please replace FIRMWARE_IMAGE with actual firmware data");
        error!("from Semtech's SWTL001 package.");
        loop {
            Timer::after(Duration::from_secs(10)).await;
        }
    }

    // Configure SPI2 for LR1110
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(8_000_000);

    let spi = Spi::new(
        p.SPI2,
        p.PB10, // SCK
        p.PC3,  // MOSI
        p.PA9,  // MISO
        p.GPDMA1_CH0,
        p.GPDMA1_CH1,
        spi_config,
    );

    let nss = Output::new(p.PD14, Level::High, Speed::VeryHigh);
    let spi_device = ExclusiveDevice::new(spi, nss, Delay).unwrap();

    // Configure LR1110 control pins
    let reset = Output::new(p.PB2, Level::High, Speed::Low);
    let busy = ExtiInput::new(p.PB13, p.EXTI13, Pull::None, Irqs);
    let dio1 = ExtiInput::new(p.PB14, p.EXTI14, Pull::Down, Irqs);

    // Optional RF switch control pins
    let rf_switch_rx: Option<Output<'_>> = None;
    let rf_switch_tx: Option<Output<'_>> = None;

    // Create InterfaceVariant
    let iv = Stm32wbaLr1110InterfaceVariant::new(reset, busy, dio1, rf_switch_rx, rf_switch_tx).unwrap();

    // Configure LR1110 chip variant
    let lr_config = lr1110_module::Config {
        chip: Lr1110Chip::new(),
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl3V0),
        use_dcdc: true,
        rx_boost: false,
    };

    // Create radio instance
    let mut radio = lr1110_module::Lr1110::new(spi_device, iv, lr_config);

    info!("Starting firmware update...");
    info!("Firmware type: {:?}", FIRMWARE_TYPE);
    info!("Expected version: 0x{:04X}", EXPECTED_FIRMWARE_VERSION);
    info!(
        "Firmware size: {} words ({} bytes)",
        FIRMWARE_IMAGE.len(),
        FIRMWARE_IMAGE.len() * 4
    );

    // Perform the firmware update
    let result = perform_firmware_update(&mut radio, FIRMWARE_TYPE, EXPECTED_FIRMWARE_VERSION, FIRMWARE_IMAGE).await;

    match result {
        Ok(FirmwareUpdateStatus::Ok) => {
            info!("==============================================");
            info!("FIRMWARE UPDATE SUCCESSFUL!");
            info!("==============================================");
            info!("The LR1110 is now running the new firmware.");
            info!("You can flash another application to use the radio.");
        }
        Ok(FirmwareUpdateStatus::WrongChipType) => {
            error!("==============================================");
            error!("FIRMWARE UPDATE FAILED: WRONG CHIP TYPE");
            error!("==============================================");
            error!("The connected chip does not match the firmware type.");
            error!("Please check the chip and firmware compatibility.");
        }
        Ok(FirmwareUpdateStatus::VerificationFailed) => {
            error!("==============================================");
            error!("FIRMWARE UPDATE FAILED: VERIFICATION ERROR");
            error!("==============================================");
            error!("The firmware was written but version check failed.");
            error!("Please retry the update.");
        }
        Ok(status) => {
            error!("Firmware update failed with status: {:?}", status);
        }
        Err(e) => {
            error!("Firmware update error: {}", e);
        }
    }

    // Loop forever
    loop {
        Timer::after(Duration::from_secs(10)).await;
    }
}

// ============================================================================
// Firmware Update Implementation
// ============================================================================

async fn perform_firmware_update<SPI, IV, C>(
    radio: &mut lr1110_module::Lr1110<SPI, IV, C>,
    firmware_type: FirmwareType,
    expected_version: u16,
    firmware_image: &[u32],
) -> Result<FirmwareUpdateStatus, &'static str>
where
    SPI: embedded_hal_async::spi::SpiDevice<u8>,
    IV: lora_phy::mod_traits::InterfaceVariant,
    C: lora_phy::lr1110::variant::Lr1110Variant,
{
    // ========================================================================
    // Step 1: Reset chip and enter bootloader mode
    // ========================================================================
    info!("Step 1: Resetting chip to enter bootloader mode...");

    // Set standby mode first
    radio.set_standby_mode(false).await.map_err(|_| "standby failed")?;
    Timer::after(Duration::from_millis(10)).await;

    // The chip should be in bootloader mode after reset
    // Wait for it to stabilize
    Timer::after(Duration::from_millis(500)).await;

    // ========================================================================
    // Step 2: Read and validate bootloader version
    // ========================================================================
    info!("Step 2: Reading bootloader version...");

    let bootloader_version: BootloaderVersion = radio
        .bootloader_get_version()
        .await
        .map_err(|_| "failed to get bootloader version")?;

    info!("  Hardware version: 0x{:02X}", bootloader_version.hw);
    info!("  Chip type: 0x{:02X}", bootloader_version.chip_type);
    info!("  Bootloader version: 0x{:04X}", bootloader_version.fw);

    // Validate chip type (0xDF = production mode)
    if bootloader_version.chip_type != 0xDF {
        error!(
            "Invalid chip type: expected 0xDF (production), got 0x{:02X}",
            bootloader_version.chip_type
        );
        return Ok(FirmwareUpdateStatus::WrongChipType);
    }

    // Validate bootloader version matches firmware type
    let expected_bootloader = firmware_type.expected_bootloader_version();
    if bootloader_version.fw != expected_bootloader {
        error!(
            "Bootloader version mismatch: expected 0x{:04X}, got 0x{:04X}",
            expected_bootloader, bootloader_version.fw
        );
        return Ok(FirmwareUpdateStatus::WrongChipType);
    }

    info!("  Bootloader version validated OK");

    // ========================================================================
    // Step 3: Read device identifiers (for logging)
    // ========================================================================
    info!("Step 3: Reading device identifiers...");

    match radio.bootloader_read_pin().await {
        Ok(pin) => info!("  PIN: {:02X}{:02X}{:02X}{:02X}", pin[0], pin[1], pin[2], pin[3]),
        Err(_) => warn!("  Failed to read PIN"),
    }

    match radio.bootloader_read_chip_eui().await {
        Ok(eui) => info!(
            "  Chip EUI: {:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            eui[0], eui[1], eui[2], eui[3], eui[4], eui[5], eui[6], eui[7]
        ),
        Err(_) => warn!("  Failed to read Chip EUI"),
    }

    match radio.bootloader_read_join_eui().await {
        Ok(eui) => info!(
            "  Join EUI: {:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            eui[0], eui[1], eui[2], eui[3], eui[4], eui[5], eui[6], eui[7]
        ),
        Err(_) => warn!("  Failed to read Join EUI"),
    }

    // ========================================================================
    // Step 4: Erase flash memory
    // ========================================================================
    info!("Step 4: Erasing flash memory...");
    info!("  This may take a few seconds...");

    radio.bootloader_erase_flash().await.map_err(|_| "flash erase failed")?;

    // Wait for erase to complete
    Timer::after(Duration::from_millis(2000)).await;

    info!("  Flash erased successfully");

    // ========================================================================
    // Step 5: Write firmware image
    // ========================================================================
    info!("Step 5: Writing firmware image...");

    let total_words = firmware_image.len();
    let total_chunks = (total_words + BOOTLOADER_FLASH_BLOCK_SIZE_WORDS - 1) / BOOTLOADER_FLASH_BLOCK_SIZE_WORDS;

    info!("  Total words: {}", total_words);
    info!("  Total chunks: {}", total_chunks);

    let mut offset: u32 = 0;
    let mut words_written: usize = 0;

    while words_written < total_words {
        // Calculate chunk size (max 64 words)
        let remaining = total_words - words_written;
        let chunk_size = remaining.min(BOOTLOADER_FLASH_BLOCK_SIZE_WORDS);

        // Get the chunk data
        let chunk = &firmware_image[words_written..words_written + chunk_size];

        // Write the chunk
        radio
            .bootloader_write_flash_encrypted(offset, chunk)
            .await
            .map_err(|_| "flash write failed")?;

        // Update progress
        words_written += chunk_size;
        offset += (chunk_size * 4) as u32;

        // Log progress every 10%
        let progress = (words_written * 100) / total_words;
        if progress % 10 == 0 {
            info!("  Progress: {}% ({}/{} words)", progress, words_written, total_words);
        }
    }

    info!("  Firmware written successfully");

    // ========================================================================
    // Step 6: Reboot and verify
    // ========================================================================
    info!("Step 6: Rebooting to execute new firmware...");

    // Reboot and exit bootloader (run from flash)
    radio.bootloader_reboot(false).await.map_err(|_| "reboot failed")?;

    // Wait for chip to boot and initialize
    Timer::after(Duration::from_millis(2000)).await;

    info!("Step 7: Verifying firmware version...");

    // Read the new firmware version
    let version: Version = radio
        .get_version()
        .await
        .map_err(|_| "failed to get version after reboot")?;

    info!("  Hardware version: 0x{:02X}", version.hw);
    info!("  Chip type: 0x{:02X}", version.chip_type);
    info!("  Firmware version: 0x{:04X}", version.fw);

    // Verify the firmware version matches expected
    if version.fw != expected_version {
        error!(
            "Firmware version mismatch: expected 0x{:04X}, got 0x{:04X}",
            expected_version, version.fw
        );
        return Ok(FirmwareUpdateStatus::VerificationFailed);
    }

    info!("  Firmware version verified OK");

    Ok(FirmwareUpdateStatus::Ok)
}
