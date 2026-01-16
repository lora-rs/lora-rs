//! LoRa P2P Receive Example
//!
//! This example demonstrates receiving LoRa packets using the LR1110 radio
//! connected to an STM32WBA65RI microcontroller via SPI.
//!
//! Hardware connections for STM32WBA65RI:
//! - SPI2_SCK:  PB10
//! - SPI2_MISO: PA9
//! - SPI2_MOSI: PC3
//! - SPI2_NSS:  PA4 (manual control via GPIO)
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
use embassy_stm32::{bind_interrupts, Config};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::lr1110::{self as lr1110_module, TcxoCtrlVoltage};
use lora_phy::lr1110::variant::Lr1110 as Lr1110Chip;
use lora_phy::mod_params::{Bandwidth, CodingRate, RxMode, SpreadingFactor};
use lora_phy::LoRa;
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

// Bind EXTI interrupts for PB13 (BUSY) and PB14 (DIO1)
bind_interrupts!(struct Irqs {
    EXTI13 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI13>;
    EXTI14 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI14>;
});

const RF_FREQUENCY: u32 = 915_000_000;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize STM32WBA65RI peripherals
    let mut config = Config::default();

    // Configure PLL1 for 96 MHz system clock
    config.rcc.pll1 = Some(embassy_stm32::rcc::Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV1,   // PLLM = 1 → HSI / 1 = 16 MHz
        mul: PllMul::MUL30,        // PLLN = 30 → 16 MHz * 30 = 480 MHz VCO
        divr: Some(PllDiv::DIV5),  // PLLR = 5 → 96 MHz (Sysclk)
        divq: None,
        divp: Some(PllDiv::DIV30), // PLLP = 30 → 16 MHz (USB)
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

    info!("STM32WBA65RI + LR1110 LoRa P2P Receive Example");

    // Configure SPI2 for LR1110
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(8_000_000);

    let spi = Spi::new(
        p.SPI2,
        p.PB10,  // SCK
        p.PC3,   // MOSI
        p.PA9,   // MISO
        p.GPDMA1_CH0,
        p.GPDMA1_CH1,
        spi_config,
    );

    let nss = Output::new(p.PA4, Level::High, Speed::VeryHigh);
    let spi_device = ExclusiveDevice::new(spi, nss, Delay).unwrap();

    // Configure LR1110 control pins
    let reset = Output::new(p.PB2, Level::High, Speed::Low);
    let busy = ExtiInput::new(p.PB13, p.EXTI13, Pull::None, Irqs);
    let dio1 = ExtiInput::new(p.PB14, p.EXTI14, Pull::Down, Irqs);

    // Optional RF switch control pins (set to None if not using)
    let rf_switch_rx: Option<Output<'_>> = None;
    let rf_switch_tx: Option<Output<'_>> = None;

    // Create InterfaceVariant
    let iv = Stm32wbaLr1110InterfaceVariant::new(
        reset,
        busy,
        dio1,
        rf_switch_rx,
        rf_switch_tx,
    )
    .unwrap();

    // Configure LR1110 chip variant
    let lr_config = lr1110_module::Config {
        chip: Lr1110Chip::new(),
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl3V0),
        use_dcdc: true,
        rx_boost: true, // Enable RX boost for better sensitivity
    };

    // Create radio instance
    let radio = lr1110_module::Lr1110::new(spi_device, iv, lr_config);

    // Create LoRa instance using the high-level API
    let mut lora = LoRa::new(radio, false, Delay).await.unwrap();

    info!("LR1110 initialized successfully");

    // Create modulation parameters (must match transmitter)
    let mdltn_params = lora
        .create_modulation_params(
            SpreadingFactor::_10,
            Bandwidth::_250KHz,
            CodingRate::_4_8,
            RF_FREQUENCY,
        )
        .unwrap();

    // Create RX packet parameters
    let rx_pkt_params = lora
        .create_rx_packet_params(
            8,     // preamble length (must match TX)
            false, // explicit header
            255,   // max payload length
            true,  // CRC on
            false, // IQ not inverted
            &mdltn_params,
        )
        .unwrap();

    info!("Listening for LoRa packets on {} Hz...", RF_FREQUENCY);

    // Receive buffer
    let mut rx_buffer = [0u8; 255];

    // Start continuous reception
    loop {
        // Prepare for reception
        match lora.prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params).await {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to prepare RX: {:?}", err);
                embassy_time::Timer::after_millis(1000).await;
                continue;
            }
        }

        // Wait for packet
        match lora.rx(&rx_pkt_params, &mut rx_buffer).await {
            Ok((received_len, packet_status)) => {
                info!(
                    "RX SUCCESS - Received {} bytes | RSSI: {} dBm | SNR: {} dB",
                    received_len, packet_status.rssi, packet_status.snr
                );

                // Print received data
                if received_len > 0 {
                    let data = &rx_buffer[..received_len as usize];
                    info!("Data: {:a}", data);

                    // Try to interpret as ASCII string
                    if let Ok(s) = core::str::from_utf8(data) {
                        info!("String: {}", s);
                    }
                }
            }
            Err(err) => {
                error!("RX FAILED: {:?}", err);
                embassy_time::Timer::after_millis(100).await;
            }
        }
    }
}
