//! This example runs on a RAK3272s board, which has a builtin Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.
#![no_std]
#![no_main]

#[path = "../iv.rs"]
mod iv;

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::spi::Spi;
use embassy_time::{Delay, Timer};
use lora_phy::LoRa;
use lora_phy::sx126x::{Stm32wl, Sx126x};
use lora_phy::{mod_params::*, sx126x};
use {defmt_rtt as _, panic_probe as _};

use self::iv::{InterruptHandler, Stm32wlInterfaceVariant, SubghzSpiDevice};

const LORA_FREQUENCY_IN_HZ: u32 = 868_000_000; // warning: set this appropriately for the region

bind_interrupts!(struct Irqs{
    SUBGHZ_RADIO => InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.msi = Some(embassy_stm32::rcc::MSIRange::RANGE48M);
        config.rcc.sys = Sysclk::MSI;
        config.rcc.mux.rngsel = mux::Rngsel::MSI;
        config.enable_debug_during_sleep = true;
    }
    let p = embassy_stm32::init(config);

    info!("config done...");
    let tx_pin = Output::new(p.PC13, Level::Low, Speed::VeryHigh);
    let rx_pin = Output::new(p.PB8, Level::Low, Speed::VeryHigh);

    let spi = Spi::new_subghz(p.SUBGHZSPI, p.DMA1_CH1, p.DMA1_CH2);
    let spi = SubghzSpiDevice(spi);
    let use_high_power_pa = true;
    let config = sx126x::Config {
        chip: Stm32wl { use_high_power_pa },
        tcxo_ctrl: None,
        use_dcdc: true,
        rx_boost: false,
    };
    let iv = Stm32wlInterfaceVariant::new(Irqs, use_high_power_pa, Some(rx_pin), Some(tx_pin), None).unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), true, Delay).await.unwrap();

    info!("lora setup done ...");

    loop {
        let mdltn_params = {
            match lora.create_modulation_params(
                SpreadingFactor::_12,
                Bandwidth::_500KHz,
                CodingRate::_4_8,
                LORA_FREQUENCY_IN_HZ,
            ) {
                Ok(mp) => mp,
                Err(err) => {
                    error!("Radio error = {}", err);
                    continue;
                }
            }
        };

        let mut tx_pkt_params = {
            match lora.create_tx_packet_params(8, false, true, false, &mdltn_params) {
                Ok(pp) => pp,
                Err(err) => {
                    error!("Radio error = {}", err);
                    continue;
                }
            }
        };
        let buffer = [0x01u8, 0x02u8, 0x03u8];

        match lora
            .prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 20, &buffer)
            .await
        {
            Ok(()) => {}
            Err(err) => {
                error!("Radio error = {}", err);
                continue;
            }
        };

        match lora.tx().await {
            Ok(()) => {
                info!("TX DONE");
            }
            Err(err) => {
                error!("Radio error = {}", err);
                continue;
            }
        };

        match lora.sleep(false).await {
            Ok(()) => info!("Sleep successful"),
            Err(err) => error!("Sleep unsuccessful = {}", err),
        }
        Timer::after_secs(10u64).await;
        info!("Waking up from sleep!");
    }
}
