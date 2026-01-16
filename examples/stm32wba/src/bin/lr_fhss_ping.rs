//! LR-FHSS Ping Demo - Port of SWDM001 lr11xx_lr_fhss_ping example
//!
//! This example demonstrates sending LR-FHSS packets using the LR1110 radio
//! connected to an STM32WBA65RI microcontroller via SPI.
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
use lora_phy::lr1110::LR_FHSS_DEFAULT_SYNC_WORD;
use lora_phy::lr1110::radio_kind_params::{
    LrFhssBandwidth, LrFhssCodingRate, LrFhssGrid, LrFhssModulationType, LrFhssParams, LrFhssV1Params, PaSelection,
};
use lora_phy::lr1110::variant::Lr1110 as Lr1110Chip;
use lora_phy::lr1110::{self as lr1110_module, TcxoCtrlVoltage};
use lora_phy::mod_params::RadioMode;
use lora_phy::mod_traits::RadioKind;
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

// Bind EXTI interrupts for PB13 (BUSY) and PB14 (DIO1)
bind_interrupts!(struct Irqs {
    EXTI13 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI13>;
    EXTI14 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI14>;
});

const RF_FREQUENCY: u32 = 915_000_000;
const TX_OUTPUT_POWER_DBM: i32 = -12; // ~half power (was -9)
const MIN_PAYLOAD_LENGTH: usize = 15;
const MAX_PAYLOAD_LENGTH: usize = 15;
const INTER_PKT_DELAY_MS: u64 = 20000;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize STM32WBA65RI peripherals
    let mut config = Config::default();

    // Configure PLL1 for 96 MHz system clock
    config.rcc.pll1 = Some(embassy_stm32::rcc::Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV1,  // PLLM = 1 → HSI / 1 = 16 MHz
        mul: PllMul::MUL30,       // PLLN = 30 → 16 MHz * 30 = 480 MHz VCO
        divr: Some(PllDiv::DIV5), // PLLR = 5 → 96 MHz (Sysclk)
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

    info!("STM32WBA65RI + LR1110 LR-FHSS Ping Example");

    // Configure SPI2 for LR1110 using valid pins for STM32WBA65RI
    // SPI2: SCK=PB10, MISO=PA9, MOSI=PC3
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
    // Use LP (Low Power) PA to match SWDM001 demo configuration
    let lr_config = lr1110_module::Config {
        chip: Lr1110Chip::with_pa(PaSelection::Lp),
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl3V0),
        use_dcdc: true,
        rx_boost: false,
    };

    // Create radio instance
    let mut lora_radio = lr1110_module::Lr1110::new(spi_device, iv, lr_config);

    // Reset and initialize the radio
    info!("Resetting LR1110...");
    lora_radio.reset(&mut Delay).await.unwrap();

    // Wait for radio to be ready
    embassy_time::Timer::after_millis(10).await;

    info!("Initializing LR-FHSS mode...");
    lora_radio.lr_fhss_init().await.unwrap();

    // Set up IRQ params for TX
    lora_radio.set_irq_params(Some(RadioMode::Transmit)).await.unwrap();

    // Set RF frequency
    lora_radio.set_channel(RF_FREQUENCY).await.unwrap();

    // Set TX power and ramp time
    lora_radio
        .set_tx_power_and_ramp_time(TX_OUTPUT_POWER_DBM, None, true)
        .await
        .unwrap();

    // Configure image calibration
    lora_radio.calibrate_image(RF_FREQUENCY).await.unwrap();

    info!("LR1110 initialized successfully");

    // Prepare LR-FHSS parameters (matching SWDM001 demo exactly)
    // Reference: SWDM001/src/demos/lr11xx_lr_fhss_ping/lr11xx_lr_fhss_ping.c
    let lr_fhss_params = LrFhssParams {
        lr_fhss_params: LrFhssV1Params {
            sync_word: LR_FHSS_DEFAULT_SYNC_WORD, // { 0x2C, 0x0F, 0x79, 0x95 }
            modulation_type: LrFhssModulationType::Gmsk488,
            coding_rate: LrFhssCodingRate::Cr5_6,
            grid: LrFhssGrid::Grid3906Hz, // LR_FHSS_V1_GRID_3906_HZ from SWDM001
            enable_hopping: true,
            bandwidth: LrFhssBandwidth::Bw136719Hz, // LR_FHSS_V1_BW_136719_HZ from SWDM001
            header_count: 2,
        },
        device_offset: 0,
    };

    // Prepare payload
    let mut payload = [0u8; 255];
    for i in 0..payload.len() {
        payload[i] = i as u8;
    }
    let mut payload_length: usize = MIN_PAYLOAD_LENGTH;

    info!(
        "Starting LR-FHSS transmission every {} seconds...",
        INTER_PKT_DELAY_MS / 1000
    );

    let mut packet_count: u32 = 0;

    // Transmit continuously
    loop {
        packet_count += 1;
        info!("Sending LR-FHSS packet #{} ({} bytes)", packet_count, payload_length);

        // Apply High ACP workaround from SWDR001 (required before TX after sleep with retention)
        // This prevents unexpectedly high adjacent channel power in LoRa transmissions.
        // Reference: SWDR001 README.md, "LR11xx firmware known limitations"
        match lora_radio.apply_high_acp_workaround().await {
            Ok(_) => {}
            Err(err) => {
                warn!("Failed to apply High ACP workaround: {:?}", err);
            }
        }

        // Initialize LR-FHSS mode (required before each transmission)
        match lora_radio.lr_fhss_init().await {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to initialize LR-FHSS: {:?}", err);
                embassy_time::Timer::after_secs(INTER_PKT_DELAY_MS / 1000).await;
                continue;
            }
        }

        // Set RF frequency and TX power (required before each transmission)
        lora_radio.set_channel(RF_FREQUENCY).await.unwrap();
        lora_radio
            .set_tx_power_and_ramp_time(TX_OUTPUT_POWER_DBM, None, true)
            .await
            .unwrap();

        // Get hop sequence ID
        let hop_sequence_count = lr1110_module::lr_fhss_get_hop_sequence_count(&lr_fhss_params);
        let hop_sequence_id = (packet_count % hop_sequence_count as u32) as u16;
        info!(
            "Using hop_sequence_id = {} (out of {} sequences)",
            hop_sequence_id, hop_sequence_count
        );

        // Build and transmit LR-FHSS frame
        match lora_radio
            .lr_fhss_build_frame(&lr_fhss_params, hop_sequence_id, &payload[0..payload_length])
            .await
        {
            Ok(_) => {
                info!("Frame built successfully");
            }
            Err(err) => {
                error!("Failed to build frame: {:?}", err);
                embassy_time::Timer::after_secs(INTER_PKT_DELAY_MS / 1000).await;
                continue;
            }
        }

        // Transmit
        match lora_radio.do_tx().await {
            Ok(_) => {
                info!("TX started, waiting for TX done...");
            }
            Err(err) => {
                error!("TX failed: {:?}", err);
                embassy_time::Timer::after_secs(INTER_PKT_DELAY_MS / 1000).await;
                continue;
            }
        }

        // Wait for TX done using EXTI interrupt on DIO1
        match lora_radio.await_irq().await {
            Ok(_) => {
                // DIO1 interrupt fired - TX is complete
                // Note: LR1110 may auto-clear IRQ flags when DIO1 triggers,
                // so we just clear any remaining flags and consider TX done
                let _ = lora_radio.process_irq_event(RadioMode::Transmit, None, true).await;
                info!("Packet sent successfully!");
            }
            Err(err) => {
                error!("Failed waiting for DIO1: {:?}", err);
                embassy_time::Timer::after_secs(INTER_PKT_DELAY_MS / 1000).await;
                continue;
            }
        }

        // Go to standby
        lora_radio.set_standby().await.unwrap();

        // Increment payload length (cycle MIN to MAX)
        if payload_length >= MAX_PAYLOAD_LENGTH {
            payload_length = MIN_PAYLOAD_LENGTH;
        } else {
            payload_length += 1;
        }

        // Wait before next transmission
        embassy_time::Timer::after_millis(INTER_PKT_DELAY_MS).await;
    }
}
