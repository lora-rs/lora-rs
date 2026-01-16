//! LR1110 System Information Example
//!
//! This example demonstrates reading system information from the LR1110 radio:
//! - Hardware and firmware version
//! - Chip type (LR1110, LR1120, LR1121)
//! - Unique device identifier (UID)
//! - Temperature sensor
//! - Battery voltage
//! - Hardware random number generator
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
    info!("LR1110 System Information Example");
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

    // Read system version
    info!("-------------------------------------------");
    info!("Version Information:");
    match radio.get_version().await {
        Ok(version) => {
            info!("  Hardware version: 0x{:02X}", version.hw);
            info!("  Chip type: {:?}", version.chip_type);
            info!(
                "  Firmware version: 0x{:04X} (v{}.{})",
                version.fw,
                version.fw_major(),
                version.fw_minor()
            );
        }
        Err(e) => {
            error!("  Failed to read version: {:?}", e);
        }
    }

    // Read unique device identifier
    info!("-------------------------------------------");
    info!("Device Identifier:");
    match radio.read_uid().await {
        Ok(uid) => {
            info!(
                "  UID: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                uid[0], uid[1], uid[2], uid[3], uid[4], uid[5], uid[6], uid[7]
            );
        }
        Err(e) => {
            error!("  Failed to read UID: {:?}", e);
        }
    }

    // Read Join EUI
    match radio.read_join_eui().await {
        Ok(join_eui) => {
            info!(
                "  Join EUI: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                join_eui[0], join_eui[1], join_eui[2], join_eui[3], join_eui[4], join_eui[5], join_eui[6], join_eui[7]
            );
        }
        Err(e) => {
            error!("  Failed to read Join EUI: {:?}", e);
        }
    }

    // Read system status
    info!("-------------------------------------------");
    info!("System Status:");
    match radio.get_status().await {
        Ok(status) => {
            info!("  Command status: {:?}", status.stat1.command_status);
            info!("  Interrupt active: {}", status.stat1.is_interrupt_active);
            info!("  Chip mode: {:?}", status.stat2.chip_mode);
            info!("  Reset status: {:?}", status.stat2.reset_status);
            info!("  Running from flash: {}", status.stat2.is_running_from_flash);
            info!("  IRQ status: 0x{:08X}", status.irq_status);
        }
        Err(e) => {
            error!("  Failed to read status: {:?}", e);
        }
    }

    // Read temperature
    info!("-------------------------------------------");
    info!("Sensors:");
    match radio.get_temp().await {
        Ok(temp_raw) => {
            // Temperature formula from datasheet
            // The raw value needs conversion - this is approximate
            info!("  Temperature (raw): {}", temp_raw);
        }
        Err(e) => {
            error!("  Failed to read temperature: {:?}", e);
        }
    }

    // Read battery voltage
    match radio.get_vbat().await {
        Ok(vbat_raw) => {
            info!("  Battery voltage (raw): {}", vbat_raw);
        }
        Err(e) => {
            error!("  Failed to read battery voltage: {:?}", e);
        }
    }

    // Generate random numbers
    info!("-------------------------------------------");
    info!("Hardware Random Number Generator:");
    for i in 0..3 {
        match radio.get_random_number().await {
            Ok(random) => {
                info!("  Random {}: 0x{:08X}", i + 1, random);
            }
            Err(e) => {
                error!("  Failed to generate random number: {:?}", e);
                break;
            }
        }
    }

    // Check for errors
    info!("-------------------------------------------");
    info!("Error Status:");
    match radio.get_errors().await {
        Ok(errors) => {
            if errors == 0 {
                info!("  No errors");
            } else {
                warn!("  Error flags: 0x{:04X}", errors);
                if errors & 0x01 != 0 {
                    warn!("    - LF RC calibration error");
                }
                if errors & 0x02 != 0 {
                    warn!("    - HF RC calibration error");
                }
                if errors & 0x04 != 0 {
                    warn!("    - ADC calibration error");
                }
                if errors & 0x08 != 0 {
                    warn!("    - PLL calibration error");
                }
                if errors & 0x10 != 0 {
                    warn!("    - Image calibration error");
                }
                if errors & 0x20 != 0 {
                    warn!("    - HF XOSC start error");
                }
                if errors & 0x40 != 0 {
                    warn!("    - LF XOSC start error");
                }
                if errors & 0x80 != 0 {
                    warn!("    - PLL lock error");
                }
            }
        }
        Err(e) => {
            error!("  Failed to read errors: {:?}", e);
        }
    }

    info!("===========================================");
    info!("System information read complete!");
    info!("===========================================");

    // Keep running
    loop {
        embassy_time::Timer::after_secs(10).await;
    }
}
