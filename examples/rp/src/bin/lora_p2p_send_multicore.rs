//! This example runs on the Raspberry Pi Pico with a Waveshare board containing a Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality using the second core, with data provided by the first core.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Executor;
use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pin, Pull};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Async, Config, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx126xInterfaceVariant;
use lora_phy::sx126x::{Sx1262, Sx126x, TcxoCtrlVoltage};
use lora_phy::LoRa;
use lora_phy::{mod_params::*, sx126x};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();
static CHANNEL: Channel<CriticalSectionRawMutex, [u8; 3], 1> = Channel::new();

const LORA_FREQUENCY_IN_HZ: u32 = 903_900_000; // warning: set this appropriately for the region

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let nss = Output::new(p.PIN_3.degrade(), Level::High);
    let reset = Output::new(p.PIN_15.degrade(), Level::High);
    let dio1 = Input::new(p.PIN_20.degrade(), Pull::None);
    let busy = Input::new(p.PIN_2.degrade(), Pull::None);

    let spi = Spi::new(
        p.SPI1,
        p.PIN_10,
        p.PIN_11,
        p.PIN_12,
        p.DMA_CH0,
        p.DMA_CH1,
        Config::default(),
    );
    let spi = ExclusiveDevice::new(spi, nss, Delay);

    let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, None, None).unwrap();

    spawn_core1(p.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let executor1 = EXECUTOR1.init(Executor::new());
        executor1.run(|spawner| unwrap!(spawner.spawn(core1_task(spi, iv))));
    });

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| unwrap!(spawner.spawn(core0_task())));
}

#[embassy_executor::task]
async fn core0_task() {
    info!("Hello from core 0");
    loop {
        CHANNEL.send([0x01u8, 0x02u8, 0x03u8]).await;
        Timer::after_millis(60 * 1000).await;
    }
}

#[embassy_executor::task]
async fn core1_task(
    spi: ExclusiveDevice<Spi<'static, SPI1, Async>, Output<'static, AnyPin>, Delay>,
    iv: GenericSx126xInterfaceVariant<Output<'static, AnyPin>, Input<'static, AnyPin>>,
) {
    info!("Hello from core 1");

    let config = sx126x::Config {
        chip: Sx1262,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: false,
    };
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), true, Delay).await.unwrap();

    let mdltn_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_10,
            Bandwidth::_250KHz,
            CodingRate::_4_8,
            LORA_FREQUENCY_IN_HZ,
        ) {
            Ok(mp) => mp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };

    let mut tx_pkt_params = {
        match lora.create_tx_packet_params(4, false, true, false, &mdltn_params) {
            Ok(pp) => pp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };

    loop {
        let buffer: [u8; 3] = CHANNEL.receive().await;
        match lora
            .prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 20, &buffer)
            .await
        {
            Ok(()) => {}
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };

        match lora.tx().await {
            Ok(()) => {
                info!("TX DONE");
            }
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };

        match lora.sleep(false).await {
            Ok(()) => info!("Sleep successful"),
            Err(err) => info!("Sleep unsuccessful = {}", err),
        }
    }
}
