//! LR1110 GNSS Scanning Example
//!
//! This example demonstrates using the built-in GNSS scanner of the LR1110 radio:
//! - Configure constellations (GPS and/or BeiDou)
//! - Set assistance position (optional, improves performance)
//! - Read and display almanac status for satellites
//! - Read individual satellite almanac data with age information
//! - Optionally update almanac from satellite signals
//! - Perform autonomous GNSS scan
//! - Read detected satellites with almanac age information
//! - Read NAV message for position solving
//!
//! The almanac age is extracted from the first byte of each satellite's almanac data.
//! A value of 0 indicates a valid (fresh) almanac, while higher values indicate older data.
//!
//! The NAV message can be sent to LoRa Cloud for position solving.
//!
//! ## Configuration
//!
//! Set your location via environment variables at build time:
//! ```bash
//! GNSS_LAT=19.6925 GNSS_LON=-98.8438 cargo run --release --bin lr1110_gnss_scan
//! ```
//!
//! If not set, uses a default location.
//!
//! ## Hardware connections for STM32WBA65RI:
//! - SPI2_SCK:  PB10
//! - SPI2_MISO: PA9
//! - SPI2_MOSI: PC3
//! - SPI2_NSS:  PD14 (manual control via GPIO)
//! - LR1110_RESET: PB2
//! - LR1110_BUSY:  PB13 (BUSY signal, active high)
//! - LR1110_DIO1:  PB14 (with EXTI interrupt)

#![no_std]
#![no_main]

#[path = "../iv.rs"]
mod iv;

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
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::lr1110::variant::Lr1110 as Lr1110Chip;
use lora_phy::lr1110::{self as lr1110_module, TcxoCtrlVoltage};
use lora_phy::lr1110::{
    GNSS_BEIDOU_MASK, GNSS_GPS_MASK, GNSS_SINGLE_ALMANAC_READ_SIZE, GnssAssistancePosition, GnssDestination,
    GnssSearchMode,
};
use lora_phy::mod_traits::RadioKind;
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

// ============================================================================
// LOCATION CONFIGURATION
// ============================================================================
// Location is set via environment variables at build time:
//   GNSS_LAT=33.4942 GNSS_LON=-111.9261 cargo run --release --bin lr1110_gnss_scan
// If not set, uses a default location.
include!(concat!(env!("OUT_DIR"), "/gnss_location.rs"));

/// Set to true to attempt almanac update from satellite before scanning.
/// This can take 60+ seconds but helps if almanac is outdated.
/// Requires clear sky view.
const UPDATE_ALMANAC_FROM_SAT: bool = true;

/// Set to true to display almanac status for sample satellites at startup.
const DISPLAY_ALMANAC_STATUS: bool = true;

/// Number of sample satellites to display almanac for (per constellation).
const ALMANAC_SAMPLE_COUNT: u8 = 4;

/// Helper to determine constellation from satellite ID
fn constellation_name(sv_id: u8) -> &'static str {
    if sv_id < 32 {
        "GPS"
    } else if sv_id >= 64 && sv_id < 128 {
        "BeiDou"
    } else {
        "Unknown"
    }
}

/// Helper to convert satellite ID to PRN
fn sv_id_to_prn(sv_id: u8) -> u8 {
    if sv_id < 32 {
        sv_id + 1 // GPS: sv_id 0-31 -> PRN 1-32
    } else if sv_id >= 64 && sv_id < 128 {
        sv_id - 64 + 1 // BeiDou: sv_id 64-127 -> PRN 1-64
    } else {
        sv_id
    }
}

/// Parse almanac age from first byte of almanac data
/// Returns age in days (0 = valid/fresh, higher = older)
fn parse_almanac_age(almanac_data: &[u8; GNSS_SINGLE_ALMANAC_READ_SIZE]) -> u8 {
    // First byte contains almanac age indicator
    // Bits 7-4: Reserved/status
    // Bits 3-0: Age in days (0 = fresh, 15 = very old or no data)
    almanac_data[0] & 0x0F
}

// ============================================================================

// Bind EXTI interrupts for PB13 (BUSY) and PB14 (DIO1)
bind_interrupts!(struct Irqs {
    EXTI13 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI13>;
    EXTI14 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI14>;
});

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

    info!("===========================================");
    info!("LR1110 GNSS Scanning Example");
    info!("===========================================");

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

    // Optional RF switch control pins (set to None if not using)
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

    info!("Initializing LR1110...");

    // Reset and initialize the radio
    radio.reset(&mut Delay).await.unwrap();
    embassy_time::Timer::after_millis(100).await;

    // Initialize system (TCXO, DC-DC, calibration) - required before GNSS operations
    info!("Configuring TCXO and calibrating...");
    radio.init_system().await.unwrap();

    // Configure RF switches for E516V02B/E516V03A board:
    // - RFSW0 (DIO5, 0x01): Sub-GHz RX path
    // - RFSW1 (DIO6, 0x02): Sub-GHz TX path
    // - RFSW2 (DIO7, 0x04): GNSS LNA enable (BGA524N6)
    // - RFSW3 (DIO8, 0x08): WiFi LNA enable (if present)
    info!("Configuring RF switches for GNSS LNA...");
    radio
        .set_dio_as_rf_switch(
            true, // enable
            0x00, // standby: no switches active
            0x01, // rx: RFSW0 (DIO5)
            0x03, // tx: RFSW0 + RFSW1 (DIO5 + DIO6)
            0x02, // tx_hp: RFSW1 (DIO6)
            0x00, // tx_hf: none
            0x04, // gnss: RFSW2 (DIO7) - enables BGA524N6 LNA
            0x08, // wifi: RFSW3 (DIO8)
        )
        .await
        .unwrap();

    // Read GNSS firmware version
    info!("-------------------------------------------");
    info!("GNSS Firmware:");
    match radio.gnss_read_firmware_version().await {
        Ok(version) => {
            info!("  GNSS Firmware: 0x{:02X}", version.gnss_firmware);
            info!("  Almanac format: 0x{:02X}", version.gnss_almanac);
        }
        Err(e) => {
            error!("  Failed to read GNSS version: {:?}", e);
        }
    }

    // Read supported constellations
    match radio.gnss_read_supported_constellations().await {
        Ok(mask) => {
            info!("  Supported constellations:");
            if mask & GNSS_GPS_MASK != 0 {
                info!("    - GPS");
            }
            if mask & GNSS_BEIDOU_MASK != 0 {
                info!("    - BeiDou");
            }
        }
        Err(e) => {
            error!("  Failed to read supported constellations: {:?}", e);
        }
    }

    // Configure constellations to use (GPS + BeiDou)
    info!("-------------------------------------------");
    info!("Configuring GNSS...");

    let constellation_mask = GNSS_GPS_MASK | GNSS_BEIDOU_MASK;
    if let Err(e) = radio.gnss_set_constellation(constellation_mask).await {
        error!("  Failed to set constellations: {:?}", e);
    } else {
        info!("  Enabled GPS and BeiDou constellations");
    }

    // Set assistance position from compile-time environment variables
    // Usage: GNSS_LAT=33.4942 GNSS_LON=-111.9261 cargo run ...
    let assistance_position = GnssAssistancePosition {
        latitude: GNSS_LATITUDE,
        longitude: GNSS_LONGITUDE,
    };

    if let Err(e) = radio.gnss_set_assistance_position(&assistance_position).await {
        error!("  Failed to set assistance position: {:?}", e);
    } else {
        info!(
            "  Set assistance position: lat={}, lon={}",
            assistance_position.latitude, assistance_position.longitude
        );
    }

    // Read back the assistance position to verify
    match radio.gnss_read_assistance_position().await {
        Ok(pos) => {
            info!("  Verified position: lat={}, lon={}", pos.latitude, pos.longitude);
        }
        Err(e) => {
            error!("  Failed to read assistance position: {:?}", e);
        }
    }

    // Get context status before scan
    info!("-------------------------------------------");
    info!("Context Status:");
    let mut needs_almanac_update = false;
    match radio.gnss_get_context_status().await {
        Ok(status) => {
            info!("  Firmware version: 0x{:02X}", status.firmware_version);
            info!("  Almanac CRC: 0x{:08X}", status.global_almanac_crc);
            info!("  Error code: {:?}", status.error_code);
            info!("  GPS almanac update needed: {}", status.almanac_update_gps);
            info!("  BeiDou almanac update needed: {}", status.almanac_update_beidou);
            needs_almanac_update = status.almanac_update_gps || status.almanac_update_beidou;
        }
        Err(e) => {
            error!("  Failed to read context status: {:?}", e);
        }
    }

    // Display almanac status for sample satellites
    if DISPLAY_ALMANAC_STATUS {
        info!("-------------------------------------------");
        info!("Almanac Status for Sample Satellites:");

        // Read almanac for sample GPS satellites (sv_id 0-31 = PRN 1-32)
        info!("  GPS Satellites:");
        for sv_id in 0..ALMANAC_SAMPLE_COUNT {
            match radio.gnss_read_almanac_per_satellite(sv_id).await {
                Ok(almanac) => {
                    let age = parse_almanac_age(&almanac);
                    let status_str = if age == 0 {
                        "Valid"
                    } else if age == 0x0F {
                        "No data"
                    } else {
                        "Stale"
                    };
                    info!(
                        "    PRN {}: Age={} days ({}), Data[0..4]={:02X}",
                        sv_id_to_prn(sv_id),
                        age,
                        status_str,
                        &almanac[0..4]
                    );
                }
                Err(e) => {
                    info!("    PRN {}: Failed to read ({:?})", sv_id_to_prn(sv_id), e);
                }
            }
        }

        // Read almanac for sample BeiDou satellites (sv_id 64-127 = PRN 1-64)
        info!("  BeiDou Satellites:");
        for i in 0..ALMANAC_SAMPLE_COUNT {
            let sv_id = 64 + i; // BeiDou sv_id starts at 64
            match radio.gnss_read_almanac_per_satellite(sv_id).await {
                Ok(almanac) => {
                    let age = parse_almanac_age(&almanac);
                    let status_str = if age == 0 {
                        "Valid"
                    } else if age == 0x0F {
                        "No data"
                    } else {
                        "Stale"
                    };
                    info!(
                        "    PRN {}: Age={} days ({}), Data[0..4]={:02X}",
                        sv_id_to_prn(sv_id),
                        age,
                        status_str,
                        &almanac[0..4]
                    );
                }
                Err(e) => {
                    info!("    PRN {}: Failed to read ({:?})", sv_id_to_prn(sv_id), e);
                }
            }
        }
    }

    // Optionally update almanac from satellite signals
    if UPDATE_ALMANAC_FROM_SAT && needs_almanac_update {
        info!("-------------------------------------------");
        info!("Updating almanac from satellite signals...");
        info!("  This may take 60+ seconds. Please ensure clear sky view.");

        if let Err(e) = radio
            .gnss_almanac_update_from_sat(constellation_mask, GnssSearchMode::HighEffort)
            .await
        {
            error!("  Failed to start almanac update: {:?}", e);
        } else {
            info!("  Almanac update started, waiting 90 seconds...");
            embassy_time::Timer::after_secs(90).await;

            // Check status after update
            match radio.gnss_get_context_status().await {
                Ok(status) => {
                    info!("  After update:");
                    info!("    Almanac CRC: 0x{:08X}", status.global_almanac_crc);
                    info!("    GPS almanac update needed: {}", status.almanac_update_gps);
                    info!("    BeiDou almanac update needed: {}", status.almanac_update_beidou);
                }
                Err(e) => {
                    error!("  Failed to read context status: {:?}", e);
                }
            }
        }
    }

    // Perform GNSS scans in a loop
    info!("===========================================");
    info!("Starting GNSS scanning loop...");
    info!("===========================================");

    let mut scan_count = 0u32;

    loop {
        scan_count += 1;
        info!("-------------------------------------------");
        info!("GNSS Scan #{}", scan_count);

        // Launch GNSS scan
        // - MidEffort: balanced scan time vs satellite detection
        // - result_mask: 0x00 for default (no extra doppler info)
        // - nb_sv_max: 0 for no limit on detected satellites
        if let Err(e) = radio.gnss_scan(GnssSearchMode::MidEffort, 0x00, 0).await {
            error!("  Failed to start GNSS scan: {:?}", e);
            embassy_time::Timer::after_secs(5).await;
            continue;
        }

        info!("  Scan started, waiting for completion...");

        // Wait for scan to complete
        // In a real application, you would wait for the GnssScanDone IRQ
        // For simplicity, we use a timeout here
        embassy_time::Timer::after_secs(10).await;

        // Get result size
        let result_size = match radio.gnss_get_result_size().await {
            Ok(size) => {
                info!("  Result size: {} bytes", size);
                size
            }
            Err(e) => {
                error!("  Failed to get result size: {:?}", e);
                embassy_time::Timer::after_secs(5).await;
                continue;
            }
        };

        if result_size == 0 {
            warn!("  No GNSS data captured");
            embassy_time::Timer::after_secs(5).await;
            continue;
        }

        // Read results
        let mut result_buffer = [0u8; 256];
        let read_size = (result_size as usize).min(result_buffer.len());

        if let Err(e) = radio.gnss_read_results(&mut result_buffer[..read_size]).await {
            error!("  Failed to read results: {:?}", e);
            embassy_time::Timer::after_secs(5).await;
            continue;
        }

        // Parse destination from first byte
        let destination = GnssDestination::from(result_buffer[0]);
        info!("  Destination: {:?}", destination);

        // Get number of detected satellites
        match radio.gnss_get_nb_satellites().await {
            Ok(nb_sv) => {
                info!("  Detected satellites: {}", nb_sv);

                // Get satellite details
                if nb_sv > 0 {
                    let mut satellites = [lora_phy::lr1110::GnssDetectedSatellite::default(); 32];
                    match radio.gnss_get_satellites(&mut satellites, nb_sv).await {
                        Ok(count) => {
                            // Display satellite details with constellation and almanac info
                            for i in 0..count as usize {
                                let sv = &satellites[i];
                                let constellation = constellation_name(sv.satellite_id);
                                let prn = sv_id_to_prn(sv.satellite_id);

                                // Try to read almanac age for this satellite
                                let almanac_info = match radio.gnss_read_almanac_per_satellite(sv.satellite_id).await {
                                    Ok(almanac) => {
                                        let age = parse_almanac_age(&almanac);
                                        if age == 0 {
                                            "Alm:OK"
                                        } else if age == 0x0F {
                                            "Alm:None"
                                        } else {
                                            "Alm:Old"
                                        }
                                    }
                                    Err(_) => "Alm:Err",
                                };

                                info!(
                                    "    {} PRN {}: C/N={}dB, Doppler={}Hz, {}",
                                    constellation, prn, sv.cnr, sv.doppler, almanac_info
                                );
                            }

                            // Summary by constellation
                            let gps_count = satellites[..count as usize]
                                .iter()
                                .filter(|s| s.satellite_id < 32)
                                .count();
                            let beidou_count = satellites[..count as usize]
                                .iter()
                                .filter(|s| s.satellite_id >= 64 && s.satellite_id < 128)
                                .count();
                            info!("  Summary: {} GPS, {} BeiDou", gps_count, beidou_count);
                        }
                        Err(e) => {
                            error!("  Failed to get satellite details: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("  Failed to get satellite count: {:?}", e);
            }
        }

        // Display NAV message (first few bytes)
        if read_size > 1 {
            info!("  NAV message (first 16 bytes):");
            let display_len = (read_size - 1).min(16);
            info!("    {:02X}", &result_buffer[1..1 + display_len]);

            // In a real application, you would send result_buffer[1..read_size]
            // to LoRa Cloud for position solving
        }

        // Wait before next scan
        info!("  Waiting 30 seconds before next scan...");
        embassy_time::Timer::after_secs(30).await;
    }
}
