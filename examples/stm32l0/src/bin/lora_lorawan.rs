//! This example runs on the STM32 LoRa Discovery board, which has a builtin Semtech Sx1276 radio.
//! It demonstrates LoRaWAN join functionality.
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::{Channel, ExtiInput};
use embassy_stm32::gpio::{Input, Level, Output, Pin, Pull, Speed};
use embassy_stm32::rng::Rng;
use embassy_stm32::time::khz;
use embassy_stm32::{bind_interrupts, peripherals, rng, spi};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx127xInterfaceVariant;
use lora_phy::lorawan_radio::LorawanRadio;
use lora_phy::sx127x::{self, Sx127x, Sx1276};
use lora_phy::LoRa;
use lorawan_device::async_device::{region, Device, EmbassyTimer, JoinMode};
use lorawan_device::default_crypto::DefaultFactory as Crypto;
use lorawan_device::{AppEui, AppKey, DevEui};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    RNG_LPUART1 => rng::InterruptHandler<peripherals::RNG>;
});

// warning: set these appropriately for the region
const LORAWAN_REGION: region::Region = region::Region::EU868;
const MAX_TX_POWER: u8 = 14;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    config.rcc.hsi = true;
    config.rcc.mux = embassy_stm32::rcc::ClockSrc::HSI;
    let p = embassy_stm32::init(config);

    let nss = Output::new(p.PA15.degrade(), Level::High, Speed::Low);
    let reset = Output::new(p.PC0.degrade(), Level::High, Speed::Low);
    let irq_pin = Input::new(p.PB4.degrade(), Pull::Up);
    let irq = ExtiInput::new(irq_pin, p.EXTI4.degrade());

    let mut spi_config = spi::Config::default();
    spi_config.frequency = khz(200);
    let spi = spi::Spi::new(p.SPI1, p.PB3, p.PA7, p.PA6, p.DMA1_CH3, p.DMA1_CH2, spi_config);
    let spi = ExclusiveDevice::new(spi, nss, Delay);

    let config = sx127x::Config {
        chip: Sx1276,
        tcxo_used: true,
        rx_boost: false,
        tx_boost: false,
    };
    let iv = GenericSx127xInterfaceVariant::new(reset, irq, None, None).unwrap();
    let lora = LoRa::new(Sx127x::new(spi, iv, config), true, Delay).await.unwrap();

    let radio: LorawanRadio<_, _, MAX_TX_POWER> = lora.into();
    let region: region::Configuration = region::Configuration::new(LORAWAN_REGION);
    let mut device: Device<_, Crypto, _, _> = Device::new(region, radio, EmbassyTimer::new(), Rng::new(p.RNG, Irqs));

    defmt::info!("Joining LoRaWAN network");

    // TODO: Adjust the EUI and Keys according to your network credentials
    let resp = device
        .join(&JoinMode::OTAA {
            deveui: DevEui::from([0, 0, 0, 0, 0, 0, 0, 0]),
            appeui: AppEui::from([0, 0, 0, 0, 0, 0, 0, 0]),
            appkey: AppKey::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        })
        .await
        .unwrap();

    info!("LoRaWAN network joined: {:?}", resp);
}
