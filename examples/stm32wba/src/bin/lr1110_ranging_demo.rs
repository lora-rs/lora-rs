//! LR1110 RTToF (Round-Trip Time of Flight) Ranging Demo
//!
//! This example demonstrates RTToF ranging between two LR1110-based devices:
//! - Manager (initiator): Sends ranging requests and calculates distance
//! - Subordinate (responder): Receives requests and sends responses
//!
//! The demo implements:
//! 1. LoRa initialization handshake to exchange configuration
//! 2. RTToF ranging with frequency hopping across 39 channels
//! 3. Distance calculation with median filtering
//!
//! Hardware connections for STM32WBA65RI:
//! - SPI2_SCK:  PB10
//! - SPI2_MISO: PA9
//! - SPI2_MOSI: PC3
//! - SPI2_NSS:  PD14 (manual control via GPIO)
//! - LR1110_RESET: PB2
//! - LR1110_BUSY:  PB13 (BUSY signal, active high)
//! - LR1110_DIO1:  PB14 (with EXTI interrupt)
//!
//! To run as Manager:
//!   cargo run --release --bin lr1110_ranging_demo --features manager
//!
//! To run as Subordinate:
//!   cargo run --release --bin lr1110_ranging_demo
//!
//! Reference: Semtech lr11xx_ranging_demo

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
use embassy_time::{Delay, Duration, Instant, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::lr1110::{self as lr1110_module, TcxoCtrlVoltage};
use lora_phy::lr1110::variant::Lr1110 as Lr1110Chip;
use lora_phy::lr1110::{
    packet_type, ranging_irq, ranging_config, ranging_channels,
    lora_sf, lora_bw, lora_cr, IrqMask,
    calculate_ranging_request_delay_ms, RttofDistanceResult,
};
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

// Bind EXTI interrupts for PB13 (BUSY) and PB14 (DIO1)
bind_interrupts!(struct Irqs {
    EXTI13 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI13>;
    EXTI14 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI14>;
});

// ============================================================================
// Configuration
// ============================================================================

/// Set to true to run as Manager (ranging initiator)
/// Set to false to run as Subordinate (ranging responder)
#[cfg(feature = "manager")]
const IS_MANAGER: bool = true;
#[cfg(not(feature = "manager"))]
const IS_MANAGER: bool = false;

/// RF frequency for initialization phase (Hz)
const RF_FREQUENCY: u32 = 915_000_000;

/// TX output power (dBm) - ~half power (was 14)
const TX_OUTPUT_POWER_DBM: i32 = 11;

/// LoRa spreading factor
const LORA_SF: u8 = lora_sf::SF8;

/// LoRa bandwidth
const LORA_BW: u8 = lora_bw::BW_500;

/// LoRa coding rate
const LORA_CR: u8 = lora_cr::CR_4_5;

/// LoRa preamble length
const LORA_PREAMBLE_LENGTH: u16 = 12;

/// Low data rate optimization (0 = off, 1 = on)
const LORA_LDRO: u8 = 0;

/// Ranging address
const RANGING_ADDRESS: u32 = ranging_config::DEFAULT_ADDRESS;

/// Number of response symbols
const RESPONSE_SYMBOLS: u8 = ranging_config::RESPONSE_SYMBOLS_COUNT;

// ============================================================================
// State Machine States
// ============================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
enum RangingState {
    /// Configure radio for LoRa initialization
    LoraConfig,
    /// Waiting for LoRa TX/RX
    LoraIdle,
    /// LoRa TX done
    LoraTxDone,
    /// LoRa RX done
    LoraRxDone,
    /// Configure radio for RTToF ranging
    RangingConfig,
    /// Start ranging on current channel
    RangingStart,
    /// Waiting for ranging result
    RangingIdle,
    /// Ranging done (exchange valid)
    RangingDone,
    /// Ranging timeout
    RangingTimeout,
    /// Ranging request valid (subordinate)
    RangingReqValid,
    /// All channels complete
    Complete,
    /// Error state
    Error,
}

// ============================================================================
// Ranging Results
// ============================================================================

/// Results from a ranging session
struct RangingResults {
    /// Raw distance results per channel (meters)
    distances: [i32; ranging_config::MAX_HOPPING_CHANNELS],
    /// RSSI per channel (dBm)
    rssi: [i8; ranging_config::MAX_HOPPING_CHANNELS],
    /// Number of successful measurements
    count: usize,
    /// Median distance (meters)
    median_distance: i32,
    /// Manager RSSI from initialization
    manager_rssi: i16,
    /// Subordinate RSSI from initialization
    subordinate_rssi: i8,
}

impl RangingResults {
    fn new() -> Self {
        Self {
            distances: [0; ranging_config::MAX_HOPPING_CHANNELS],
            rssi: [0; ranging_config::MAX_HOPPING_CHANNELS],
            count: 0,
            median_distance: 0,
            manager_rssi: 0,
            subordinate_rssi: 0,
        }
    }

    /// Calculate median distance from collected results
    fn calculate_median(&mut self) {
        if self.count == 0 {
            return;
        }

        // Copy distances for sorting
        let mut sorted = [0i32; ranging_config::MAX_HOPPING_CHANNELS];
        sorted[..self.count].copy_from_slice(&self.distances[..self.count]);

        // Simple bubble sort
        for i in (1..self.count).rev() {
            for j in 0..i {
                if sorted[j] > sorted[j + 1] {
                    sorted.swap(j, j + 1);
                }
            }
        }

        // Calculate median
        if self.count % 2 == 0 {
            self.median_distance = (sorted[self.count / 2] + sorted[self.count / 2 - 1]) / 2;
        } else {
            self.median_distance = sorted[self.count / 2];
        }
    }

    /// Calculate Packet Error Rate (percentage)
    fn per(&self) -> u8 {
        100 - ((self.count * 100) / ranging_config::MAX_HOPPING_CHANNELS) as u8
    }
}

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

    if IS_MANAGER {
        info!("=== LR1110 Ranging Demo - MANAGER Mode ===");
    } else {
        info!("=== LR1110 Ranging Demo - SUBORDINATE Mode ===");
    }

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
        rx_boost: true,
    };

    // Create radio instance
    let mut radio = lr1110_module::Lr1110::new(spi_device, iv, lr_config);

    info!("Initializing LR1110...");

    // Reset the radio
    radio.set_standby_mode(false).await.unwrap();
    Timer::after(Duration::from_millis(10)).await;

    info!("LR1110 initialized successfully");

    // Run the ranging demo
    loop {
        let result = run_ranging_session(&mut radio).await;
        match result {
            Ok(results) => {
                info!("=== Ranging Session Complete ===");
                info!("Successful measurements: {}/{}", results.count, ranging_config::MAX_HOPPING_CHANNELS);
                info!("Packet Error Rate: {}%", results.per());
                if results.count >= ranging_config::MIN_HOPPING_CHANNELS {
                    info!("Median distance: {} meters", results.median_distance);
                    info!("Manager RSSI: {} dBm", results.manager_rssi);
                    info!("Subordinate RSSI: {} dBm", results.subordinate_rssi);
                } else {
                    warn!("Not enough measurements for valid result (min: {})", ranging_config::MIN_HOPPING_CHANNELS);
                }
                info!("================================");
            }
            Err(e) => {
                error!("Ranging session failed: {:?}", e);
            }
        }

        // Wait before next session
        Timer::after(Duration::from_secs(3)).await;
    }
}

// ============================================================================
// Ranging Session Implementation
// ============================================================================

async fn run_ranging_session<SPI, IV, C>(
    radio: &mut lr1110_module::Lr1110<SPI, IV, C>,
) -> Result<RangingResults, &'static str>
where
    SPI: embedded_hal_async::spi::SpiDevice<u8>,
    IV: lora_phy::mod_traits::InterfaceVariant,
    C: lora_phy::lr1110::variant::Lr1110Variant,
{
    let mut state = RangingState::LoraConfig;
    let mut results = RangingResults::new();
    let mut current_channel: usize = 0;
    let mut tx_buffer = [0u8; ranging_config::INIT_PAYLOAD_LENGTH];

    // Calculate timing parameters
    let ranging_delay_ms = calculate_ranging_request_delay_ms(
        LORA_BW,
        LORA_SF,
        LORA_PREAMBLE_LENGTH,
        RESPONSE_SYMBOLS,
    );
    let global_timeout_ms = ranging_delay_ms * (ranging_config::MAX_HOPPING_CHANNELS as u32 + 1)
        + (ranging_config::MAX_HOPPING_CHANNELS as u32) + 5;

    info!("Ranging delay per channel: {} ms", ranging_delay_ms);
    info!("Global timeout: {} ms", global_timeout_ms);

    let session_start = Instant::now();
    let mut channel_start = Instant::now();

    loop {
        // Check global timeout
        if session_start.elapsed() > Duration::from_millis(global_timeout_ms as u64) {
            error!("Global ranging timeout!");
            return Err("Global timeout");
        }

        match state {
            RangingState::LoraConfig => {
                info!("Configuring radio for LoRa initialization...");

                // Set packet type to LoRa
                radio.set_packet_type(packet_type::LORA).await.map_err(|_| "set_packet_type")?;

                // Set frequency
                radio.set_rf_frequency(RF_FREQUENCY).await.map_err(|_| "set_rf_frequency")?;

                // Set modulation parameters
                radio.set_lora_mod_params(LORA_SF, LORA_BW, LORA_CR, LORA_LDRO).await.map_err(|_| "set_lora_mod_params")?;

                // Set packet parameters
                radio.set_lora_pkt_params(
                    LORA_PREAMBLE_LENGTH,
                    0, // explicit header
                    ranging_config::INIT_PAYLOAD_LENGTH as u8,
                    1, // CRC on
                    0, // IQ not inverted
                ).await.map_err(|_| "set_lora_pkt_params")?;

                // Set sync word
                radio.set_lora_sync_word(ranging_config::LORA_SYNC_WORD).await.map_err(|_| "set_lora_sync_word")?;

                // Set IRQ params for LoRa
                radio.set_dio_irq_params_custom(ranging_irq::LORA_IRQ_MASK).await.map_err(|_| "set_dio_irq_params")?;
                radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;

                if IS_MANAGER {
                    // Manager: Send initialization packet
                    tx_buffer[0] = ((RANGING_ADDRESS >> 24) & 0xFF) as u8;
                    tx_buffer[1] = ((RANGING_ADDRESS >> 16) & 0xFF) as u8;
                    tx_buffer[2] = ((RANGING_ADDRESS >> 8) & 0xFF) as u8;
                    tx_buffer[3] = (RANGING_ADDRESS & 0xFF) as u8;
                    tx_buffer[4] = 0; // First channel
                    tx_buffer[5] = 0;

                    info!("Sending initialization packet...");
                    radio.write_tx_buffer(0, &tx_buffer).await.map_err(|_| "write_tx_buffer")?;
                    radio.set_tx_mode(0).await.map_err(|_| "set_tx_mode")?;
                } else {
                    // Subordinate: Wait for initialization packet
                    info!("Waiting for initialization packet...");
                    radio.set_rx_mode(ranging_config::RX_CONTINUOUS).await.map_err(|_| "set_rx_mode")?;
                }

                state = RangingState::LoraIdle;
            }

            RangingState::LoraIdle => {
                // Wait for IRQ
                Timer::after(Duration::from_millis(1)).await;

                let irq_flags = radio.get_irq_flags().await.map_err(|_| "get_irq_flags")?;

                if IrqMask::TxDone.is_set(irq_flags) {
                    radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                    state = RangingState::LoraTxDone;
                } else if IrqMask::RxDone.is_set(irq_flags) {
                    radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                    state = RangingState::LoraRxDone;
                } else if IrqMask::Timeout.is_set(irq_flags) {
                    radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                    warn!("LoRa timeout");
                    return Err("LoRa timeout");
                } else if IrqMask::CrcError.is_set(irq_flags) || IrqMask::HeaderError.is_set(irq_flags) {
                    radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                    warn!("LoRa CRC/Header error");
                    // Continue waiting
                }
            }

            RangingState::LoraTxDone => {
                info!("LoRa TX done");
                radio.set_standby_mode(true).await.map_err(|_| "set_standby_mode")?;

                if IS_MANAGER {
                    // Manager: Wait for subordinate response
                    info!("Waiting for subordinate response...");
                    radio.set_rx_mode(100 * 32768 / 1000).await.map_err(|_| "set_rx_mode")?; // 100ms timeout
                    state = RangingState::LoraIdle;
                } else {
                    // Subordinate: Configure for RTToF ranging
                    state = RangingState::RangingConfig;
                }
            }

            RangingState::LoraRxDone => {
                info!("LoRa RX done");
                radio.set_standby_mode(true).await.map_err(|_| "set_standby_mode")?;

                // Read received packet
                let (pld_len, start_ptr) = radio.get_rx_buffer_status().await.map_err(|_| "get_rx_buffer_status")?;
                let mut rx_buffer = [0u8; ranging_config::INIT_PAYLOAD_LENGTH];
                radio.read_rx_buffer(start_ptr, pld_len.min(ranging_config::INIT_PAYLOAD_LENGTH as u8), &mut rx_buffer).await.map_err(|_| "read_rx_buffer")?;

                // Verify address
                let rx_address = ((rx_buffer[0] as u32) << 24)
                    | ((rx_buffer[1] as u32) << 16)
                    | ((rx_buffer[2] as u32) << 8)
                    | (rx_buffer[3] as u32);

                if rx_address != RANGING_ADDRESS {
                    warn!("Address mismatch: expected {:08X}, got {:08X}", RANGING_ADDRESS, rx_address);
                    state = RangingState::LoraConfig;
                    continue;
                }

                // Get packet status
                let (rssi, _snr) = radio.get_lora_packet_status().await.map_err(|_| "get_lora_packet_status")?;

                if IS_MANAGER {
                    // Manager: Got subordinate's response
                    results.subordinate_rssi = rx_buffer[5] as i8;
                    results.manager_rssi = rssi;
                    current_channel = rx_buffer[4] as usize;
                    info!("Received subordinate response, subordinate RSSI: {} dBm", results.subordinate_rssi);
                    state = RangingState::RangingConfig;
                } else {
                    // Subordinate: Got manager's initialization
                    current_channel = rx_buffer[4] as usize;
                    results.manager_rssi = rssi;

                    // Send response with our RSSI
                    tx_buffer.copy_from_slice(&rx_buffer);
                    tx_buffer[5] = (rssi as i8) as u8;

                    info!("Sending response with RSSI: {} dBm", rssi);
                    radio.write_tx_buffer(0, &tx_buffer).await.map_err(|_| "write_tx_buffer")?;
                    radio.set_tx_mode(0).await.map_err(|_| "set_tx_mode")?;
                    state = RangingState::LoraIdle;
                }
            }

            RangingState::RangingConfig => {
                info!("Configuring radio for RTToF ranging...");

                // Set packet type to RTToF
                radio.set_packet_type(packet_type::RTTOF).await.map_err(|_| "set_packet_type")?;

                // Set modulation parameters (same as LoRa)
                radio.set_lora_mod_params(LORA_SF, LORA_BW, LORA_CR, LORA_LDRO).await.map_err(|_| "set_lora_mod_params")?;

                // Set packet parameters with larger payload for RTToF
                radio.set_lora_pkt_params(
                    LORA_PREAMBLE_LENGTH,
                    0, // explicit header
                    10, // RTToF payload length
                    1, // CRC on
                    0, // IQ not inverted
                ).await.map_err(|_| "set_lora_pkt_params")?;

                // Configure RTToF parameters
                radio.rttof_set_parameters(RESPONSE_SYMBOLS).await.map_err(|_| "rttof_set_parameters")?;

                // Set RX/TX delay indicator (calibration value - simplified)
                // A proper implementation would use a lookup table based on SF/BW
                radio.rttof_set_rx_tx_delay_indicator(0).await.map_err(|_| "rttof_set_rx_tx_delay_indicator")?;

                if IS_MANAGER {
                    // Manager: Set request address
                    radio.rttof_set_request_address(RANGING_ADDRESS).await.map_err(|_| "rttof_set_request_address")?;
                    radio.set_dio_irq_params_custom(ranging_irq::MANAGER_IRQ_MASK).await.map_err(|_| "set_dio_irq_params")?;
                } else {
                    // Subordinate: Set address
                    radio.rttof_set_address(RANGING_ADDRESS, ranging_config::SUBORDINATE_CHECK_LENGTH_BYTES).await.map_err(|_| "rttof_set_address")?;
                    radio.set_dio_irq_params_custom(ranging_irq::SUBORDINATE_IRQ_MASK).await.map_err(|_| "set_dio_irq_params")?;
                }

                radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;

                current_channel = 0;
                results.count = 0;
                channel_start = Instant::now();

                state = RangingState::RangingStart;
            }

            RangingState::RangingStart => {
                // Wait for channel delay
                let elapsed = channel_start.elapsed().as_millis() as u32;
                if elapsed < ranging_delay_ms && current_channel > 0 {
                    Timer::after(Duration::from_millis(1)).await;
                    continue;
                }

                if current_channel >= ranging_config::MAX_HOPPING_CHANNELS {
                    state = RangingState::Complete;
                    continue;
                }

                // Set frequency for this channel
                let freq = ranging_channels::US915[current_channel];
                radio.set_rf_frequency(freq).await.map_err(|_| "set_rf_frequency")?;

                channel_start = Instant::now();

                if IS_MANAGER {
                    // Manager: Send ranging request
                    radio.set_tx_mode(0).await.map_err(|_| "set_tx_mode")?;
                } else {
                    // Subordinate: Wait for ranging request
                    let timeout_rtc = (ranging_delay_ms * 32768 / 1000) as u32;
                    radio.set_rx_mode(timeout_rtc).await.map_err(|_| "set_rx_mode")?;
                }

                state = RangingState::RangingIdle;
            }

            RangingState::RangingIdle => {
                // Wait for IRQ
                Timer::after(Duration::from_millis(1)).await;

                let irq_flags = radio.get_irq_flags().await.map_err(|_| "get_irq_flags")?;

                if IS_MANAGER {
                    // Manager: Check for exchange valid or timeout
                    if IrqMask::RttofExchValid.is_set(irq_flags) {
                        radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                        state = RangingState::RangingDone;
                    } else if IrqMask::RttofTimeout.is_set(irq_flags) {
                        radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                        state = RangingState::RangingTimeout;
                    }
                } else {
                    // Subordinate: Check for request valid, response done, or discarded
                    if IrqMask::RttofReqValid.is_set(irq_flags) {
                        radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                        state = RangingState::RangingReqValid;
                    } else if IrqMask::RttofRespDone.is_set(irq_flags) {
                        radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                        state = RangingState::RangingDone;
                    } else if IrqMask::RttofReqDiscarded.is_set(irq_flags) {
                        radio.clear_all_irq().await.map_err(|_| "clear_all_irq")?;
                        debug!("Request discarded (address mismatch)");
                        state = RangingState::RangingStart;
                    }

                    // Check for software timeout (subordinate doesn't get hardware timeout)
                    if channel_start.elapsed() > Duration::from_millis(ranging_delay_ms as u64) {
                        state = RangingState::RangingTimeout;
                    }
                }
            }

            RangingState::RangingDone => {
                radio.set_standby_mode(true).await.map_err(|_| "set_standby_mode")?;

                if IS_MANAGER {
                    // Manager: Get ranging result
                    let result: RttofDistanceResult = radio.rttof_get_distance_result(
                        lora_phy::mod_params::Bandwidth::_500KHz
                    ).await.map_err(|_| "rttof_get_distance_result")?;

                    results.distances[results.count] = result.distance_m;
                    results.rssi[results.count] = result.rssi_dbm;
                    results.count += 1;

                    debug!("Ch {}: {} m, {} dBm", current_channel, result.distance_m, result.rssi_dbm);
                }

                current_channel += 1;
                channel_start = Instant::now();
                state = RangingState::RangingStart;
            }

            RangingState::RangingTimeout => {
                radio.set_standby_mode(true).await.map_err(|_| "set_standby_mode")?;
                debug!("Ch {}: timeout", current_channel);
                current_channel += 1;
                channel_start = Instant::now();
                state = RangingState::RangingStart;
            }

            RangingState::RangingReqValid => {
                // Subordinate received valid request, response will be sent automatically
                // Wait for response done
                state = RangingState::RangingIdle;
            }

            RangingState::Complete => {
                info!("Ranging complete, {} measurements collected", results.count);
                results.calculate_median();
                return Ok(results);
            }

            RangingState::Error => {
                return Err("State machine error");
            }
        }
    }
}
