//! LoRa P2P Receive Example for STM32WBA65RI + LR1110
//!
//! This example demonstrates receiving LoRa packets using the LR1110 radio
//! connected to an STM32WBA65RI microcontroller via SPI.
//!
//! Hardware connections (adjust pin numbers for your board):
//! - SPI1_NSS:  PA4
//! - SPI1_SCK:  PB4
//! - SPI1_MISO: PA11
//! - SPI1_MOSI: PA12
//! - LR1110_RESET: PB2
//! - LR1110_DIO1:  PB1 (with EXTI interrupt)
//! - RF_SWITCH_RX: PC6 (optional)
//! - RF_SWITCH_TX: PC7 (optional)

#![no_std]
#![no_main]

#[path = "../iv.rs"]
mod iv;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericLr1110InterfaceVariant;
use lora_phy::lr1110::{Lr1110, TcxoCtrlVoltage};
use lora_phy::mod_params::*;
use lora_phy::LoRa;
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

// LoRa configuration
const LORA_FREQUENCY_IN_HZ: u32 = 915_000_000; // US915 - adjust for your region

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize STM32WBA peripherals
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        // Configure HSE and PLL for 100MHz system clock
        config.rcc.hse = Some(Hse {
            freq: Hertz(32_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.sys = Sysclk::Pll1R;
        config.rcc.pll1 = Some(Pll {
            source: PllSource::Hse,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL50,
            divp: None,
            divq: None,
            divr: Some(PllRDiv::DIV4), // 32MHz / 4 * 50 / 4 = 100MHz
        });
    }
    let p = embassy_stm32::init(config);

    info!("STM32WBA65RI + LR1110 LoRa P2P Receive Example");

    // Configure SPI for LR1110
    // SPI1: SCK=PB4, MISO=PA11, MOSI=PA12, NSS=PA4
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(8_000_000); // 8 MHz SPI clock

    let spi = Spi::new(
        p.SPI1,
        p.PB4,  // SCK
        p.PA12, // MOSI
        p.PA11, // MISO
        p.DMA1_CH0,
        p.DMA1_CH1,
        spi_config,
    );

    let nss = Output::new(p.PA4, Level::High, Speed::VeryHigh);
    let spi_device = ExclusiveDevice::new(spi, nss, Delay);

    // Configure LR1110 control pins
    let reset = Output::new(p.PB2, Level::High, Speed::Low);
    let dio1 = ExtiInput::new(p.PB1, p.EXTI1, Pull::Down);

    // Optional RF switch control pins
    let rf_switch_rx = Some(Output::new(p.PC6, Level::Low, Speed::Low));
    let rf_switch_tx = Some(Output::new(p.PC7, Level::Low, Speed::Low));

    // Create InterfaceVariant
    let iv = Stm32wbaLr1110InterfaceVariant::new(
        reset,
        dio1,
        rf_switch_rx,
        rf_switch_tx,
    )
    .unwrap();

    // Configure LR1110
    let config = lora_phy::lr1110::Config {
        chip: lora_phy::lr1110::Lr1110::new(),
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V8), // Adjust based on your board
        use_dcdc: true,  // Use DCDC for better efficiency
        rx_boost: false,
    };

    // Create LoRa instance
    let mut lora = LoRa::new(
        Lr1110::new(spi_device, iv, config),
        false, // private network
        Delay,
    )
    .await
    .unwrap();

    info!("LR1110 initialized successfully");

    // Create modulation parameters (must match transmitter)
    let mdltn_params = lora
        .create_modulation_params(
            SpreadingFactor::_10,
            Bandwidth::_125KHz,
            CodingRate::_4_5,
            LORA_FREQUENCY_IN_HZ,
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

    info!("Listening for packets on {} Hz...", LORA_FREQUENCY_IN_HZ);

    // Prepare for reception
    lora.prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
        .await
        .unwrap();

    // Receive buffer
    let mut receiving_buffer = [0u8; 255];

    // Start continuous reception
    loop {
        match lora.rx(&rx_pkt_params, &mut receiving_buffer).await {
            Ok((received_len, packet_status)) => {
                info!(
                    "✓ RX SUCCESS - Received {} bytes | RSSI: {} dBm | SNR: {} dB",
                    received_len, packet_status.rssi, packet_status.snr
                );

                // Print received data
                if received_len > 0 {
                    let data = &receiving_buffer[..received_len as usize];
                    info!("Data: {:?}", data);

                    // Try to interpret as ASCII string
                    if let Ok(s) = core::str::from_utf8(data) {
                        info!("String: {}", s);
                    }
                }
            }
            Err(err) => {
                error!("✗ RX FAILED: {:?}", err);

                // Re-prepare for next reception
                lora.prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
                    .await
                    .unwrap();
            }
        }
    }
}
