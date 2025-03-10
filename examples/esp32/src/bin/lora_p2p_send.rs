//! This example runs on the Heltec WiFi LoRa ESP32 board, which has a builtin Semtech Sx1276 radio.
//! It demonstrates LORA P2P send functionality.
#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Delay;
use esp_hal::gpio::Input;
use esp_hal::gpio::InputConfig;
use esp_hal::gpio::Level;
use esp_hal::gpio::Output;
use esp_hal::gpio::OutputConfig;
use esp_hal::spi::master::Config;
use esp_hal::spi::master::Spi;
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use esp_hal::Async;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use esp_println as _;
use lora_phy::iv::GenericSx126xInterfaceVariant;
use lora_phy::mod_params::Bandwidth;
use lora_phy::mod_params::CodingRate;
use lora_phy::mod_params::SpreadingFactor;
use lora_phy::sx126x;
use lora_phy::sx126x::Sx1262;
use lora_phy::sx126x::Sx126x;
use lora_phy::sx126x::TcxoCtrlVoltage;
use lora_phy::LoRa;
use static_cell::StaticCell;

const LORA_FREQUENCY_IN_HZ: u32 = 903_900_000; // WARNING: Set this appropriately for the region

static SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, esp_hal::spi::master::Spi<'static, Async>>> =
    StaticCell::new();

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    // Set up ESP32
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let timer_group = TimerGroup::new(peripherals.TIMG0);

    esp_hal_embassy::init(timer_group.timer1);

    // Initialize SPI
    let nss = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());
    let sclk = peripherals.GPIO9;
    let mosi = peripherals.GPIO10;
    let miso = peripherals.GPIO11;

    let reset = Output::new(peripherals.GPIO12, Level::Low, OutputConfig::default());
    let busy = Input::new(peripherals.GPIO13, InputConfig::default());
    let dio1 = Input::new(peripherals.GPIO14, InputConfig::default());

    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(Rate::from_khz(100))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_miso(miso)
    .into_async();

    // Initialize the static SPI bus
    let spi_bus = SPI_BUS.init(Mutex::new(spi));
    let spi_device = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice::new(spi_bus, nss);

    // Create the SX126x configuration
    let sx126x_config = sx126x::Config {
        chip: Sx1262,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: false,
        rx_boost: true,
    };

    // Create the radio instance
    let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, None, None).unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi_device, iv, sx126x_config), false, Delay)
        .await
        .unwrap();

    let modulation_params = {
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

    let mut tx_packet_params = {
        match lora.create_tx_packet_params(4, false, true, false, &modulation_params) {
            Ok(pp) => pp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };

    let buffer = b"hello";

    match lora
        .prepare_for_tx(&modulation_params, &mut tx_packet_params, 20, buffer)
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

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defmt::info!("Panic: {}", info);
    loop {}
}
