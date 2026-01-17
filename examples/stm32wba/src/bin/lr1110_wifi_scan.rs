//! LR1110 WiFi Passive Scanning Example
//!
//! This example demonstrates using the built-in WiFi scanner of the LR1110 radio:
//! - Configure scan parameters (signal type, channels, scan mode)
//! - Perform WiFi passive scan to detect access points
//! - Read detected AP information (MAC address, RSSI, channel)
//!
//! The results can be sent to LoRa Cloud for WiFi-based geolocation.
//!
//! Hardware connections for STM32WBA65RI:
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
    WIFI_ALL_CHANNELS_MASK, WIFI_MAX_RESULTS, WifiBasicMacTypeChannelResult, WifiScanMode, WifiSignalTypeScan,
};
use lora_phy::mod_traits::RadioKind;
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

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
    info!("LR1110 WiFi Passive Scanning Example");
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

    // Initialize system (TCXO, DC-DC, calibration)
    info!("Configuring TCXO and calibrating...");
    radio.init_system().await.unwrap();

    // Note: WiFi path on this board has no RF switch - directly connected via BGA524N6 LNA

    // Read WiFi firmware version
    info!("-------------------------------------------");
    info!("WiFi Firmware:");
    match radio.wifi_read_version().await {
        Ok(version) => {
            info!("  WiFi Firmware: v{}.{}", version.major, version.minor);
        }
        Err(e) => {
            error!("  Failed to read WiFi version: {:?}", e);
        }
    }

    // Perform WiFi scans in a loop
    info!("===========================================");
    info!("Starting WiFi scanning loop...");
    info!("===========================================");

    let mut scan_count = 0u32;

    loop {
        scan_count += 1;
        info!("-------------------------------------------");
        info!("WiFi Scan #{}", scan_count);

        // Reset cumulative timing before scan
        if let Err(e) = radio.wifi_reset_cumulative_timing().await {
            warn!("  Failed to reset timing: {:?}", e);
        }

        // Launch WiFi scan
        // - TypeBGN: Scan for all WiFi types (802.11 b/g/n)
        // - ALL_CHANNELS_MASK: Scan all 14 channels
        // - Beacon: Basic scan mode for Beacons and Probe Responses
        // - max_results: 32 (maximum allowed, range is 1-32, 0 is forbidden!)
        // - nb_scan_per_channel: 10 scans per channel
        // - timeout_per_scan_ms: 90ms per scan
        // - abort_on_timeout: false (continue scanning on timeout)
        if let Err(e) = radio
            .wifi_scan(
                WifiSignalTypeScan::TypeBGN,
                WIFI_ALL_CHANNELS_MASK,
                WifiScanMode::Beacon,
                32,    // max_results (range 1-32, 0 is forbidden!)
                10,    // nb_scan_per_channel
                90,    // timeout_per_scan_ms
                false, // abort_on_timeout
            )
            .await
        {
            error!("  Failed to start WiFi scan: {:?}", e);
            embassy_time::Timer::after_secs(5).await;
            continue;
        }

        info!("  Scan started, waiting for completion...");

        // Wait for scan to complete
        // In a real application, you would wait for the WifiScanDone IRQ
        // For simplicity, we use a timeout here
        // (14 channels * 10 scans * 90ms = ~12.6 seconds max)
        embassy_time::Timer::after_secs(15).await;

        // Get number of results
        let nb_results = match radio.wifi_get_nb_results().await {
            Ok(count) => {
                info!("  Detected {} access points", count);
                count
            }
            Err(e) => {
                error!("  Failed to get result count: {:?}", e);
                embassy_time::Timer::after_secs(5).await;
                continue;
            }
        };

        if nb_results == 0 {
            warn!("  No WiFi access points detected");
            embassy_time::Timer::after_secs(5).await;
            continue;
        }

        // Read results
        let mut results = [WifiBasicMacTypeChannelResult::default(); WIFI_MAX_RESULTS];
        let read_count = match radio
            .wifi_read_basic_mac_type_channel_results(&mut results, 0, nb_results.min(WIFI_MAX_RESULTS as u8))
            .await
        {
            Ok(count) => count,
            Err(e) => {
                error!("  Failed to read results: {:?}", e);
                embassy_time::Timer::after_secs(5).await;
                continue;
            }
        };

        // Display results
        info!("  Results:");
        for i in 0..read_count as usize {
            let result = &results[i];
            let mac = result.mac_address;
            let channel = result.channel();
            let rssi = result.rssi;
            let signal_type = result.signal_type();
            let rssi_valid = if result.rssi_valid() { "" } else { " (invalid)" };

            info!(
                "    AP {}: MAC={:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} Ch={:?} RSSI={}dBm{} Type={:?}",
                i + 1,
                mac[0],
                mac[1],
                mac[2],
                mac[3],
                mac[4],
                mac[5],
                channel,
                rssi,
                rssi_valid,
                signal_type
            );
        }

        // Read cumulative timing
        match radio.wifi_read_cumulative_timing().await {
            Ok(timing) => {
                info!("  Timing:");
                info!("    RX detection:   {} us", timing.rx_detection_us);
                info!("    RX correlation: {} us", timing.rx_correlation_us);
                info!("    RX capture:     {} us", timing.rx_capture_us);
                info!("    Demodulation:   {} us", timing.demodulation_us);
            }
            Err(e) => {
                warn!("  Failed to read timing: {:?}", e);
            }
        }

        // In a real application, you would send the MAC addresses and RSSI values
        // to LoRa Cloud for WiFi-based geolocation

        // Wait before next scan
        info!("  Waiting 30 seconds before next scan...");
        embassy_time::Timer::after_secs(30).await;
    }
}
