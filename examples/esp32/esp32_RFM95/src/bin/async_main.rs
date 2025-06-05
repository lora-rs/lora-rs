#![no_std]
#![no_main]

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Delay;
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_println as _;
use static_cell::StaticCell;

#[path = "../iv.rs"]
mod iv;
use crate::iv::LoraWanSx127xInterfaceVariant;

use esp_hal::{
    clock::CpuClock,
    gpio::{Input, Level, Output, Pull},
    rng::Rng,
    spi::{
        master::{Config, Spi},
        Mode,
    },
    time::RateExtU32,
    Async,
};

use lora_phy::{
    lorawan_radio::LorawanRadio,
    sx127x::{self, Sx1276, Sx127x},
    LoRa,
};

use lorawan_device::{
    async_device::{region, Device, EmbassyTimer, JoinMode},
    AppEui, AppKey, DevEui,
};

const LORAWAN_REGION: region::Region = region::Region::EU868;
const MAX_TX_POWER: u8 = 14;

static SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, esp_hal::spi::master::Spi<'static, Async>>> =
    StaticCell::new();

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    // Pin configuration
    let reset = Output::new(peripherals.GPIO17, Level::High);
    let dio0 = Input::new(peripherals.GPIO21, Pull::None);
    let dio1 = Input::new(peripherals.GPIO22, Pull::None);
    let sclk = peripherals.GPIO5;
    let miso = peripherals.GPIO19;
    let mosi = peripherals.GPIO23;
    let nss = Output::new(peripherals.GPIO18, Level::High);

    // SPI configuration
    let spi = Spi::new(
        peripherals.SPI2,
        Config::default().with_frequency(100.kHz()).with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_miso(miso)
    .into_async();

    let config = sx127x::Config {
        chip: Sx1276,
        tcxo_used: false,
        rx_boost: false,
        tx_boost: true, // IMPORTANT: must be TRUE for the RFM95 module to work reliably.
    };

    let spi_bus = SPI_BUS.init(Mutex::new(spi));
    let spi_device = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice::new(spi_bus, nss);

    // LoRa device configuration.
    let iv = LoraWanSx127xInterfaceVariant::new(reset, dio0, dio1).unwrap();
    let radio = Sx127x::new(spi_device, iv, config);
    let lora = LoRa::new(radio, true, Delay).await.unwrap();

    let radio: LorawanRadio<_, _, MAX_TX_POWER> = lora.into();
    let region = region::Configuration::new(LORAWAN_REGION);
    let mut device: Device<_, _, _> = Device::new(region, radio, EmbassyTimer::new(), Rng::new(peripherals.RNG));

    info!("Starting...");

    // Try to join LoRaWAN network.
    loop {
        let response = device
            .join(&JoinMode::OTAA {
                deveui: DevEui::from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // Replace with your own.
                appkey: AppKey::from([
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, // Replace with your own.
                ]),
                appeui: AppEui::from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // Can be left as-is in most cases.
            })
            .await;

        match response {
            Ok(response) => match response {
                lorawan_device::async_device::JoinResponse::JoinSuccess => {
                    info!("LoRaWAN network joined succesfully!");
                    break;
                }
                lorawan_device::async_device::JoinResponse::NoJoinAccept => {
                    error!("No join accept from LoRaWAN network");
                }
            },
            Err(err) => {
                error!("{}", err);
                continue;
            }
        };

        Delay.delay_ms(5000);
    }

    // Begin transmissions after a succesful join.
    let mut counter = 1u32;
    loop {
        info!("Counter {}", counter);
        let buffer = counter.to_be_bytes();

        let result = device.send(&buffer, 1, true).await;
        info!("{}", result);

        counter += 1;
        Delay.delay_ms(10000);
    }
}
