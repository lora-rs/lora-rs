//! This example runs on the STM32 LoRa Discovery board (B-L072Z-LRWAN1), which
//! has a builtin Semtech Sx1276 radio inside the Murata CMWX1ZZABZ module.
//! It demonstrates LoRaWAN join functionality.
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::rng::Rng;
use embassy_stm32::time::khz;
use embassy_stm32::{bind_interrupts, dma, peripherals, rng, spi};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx127xInterfaceVariant;
use lora_phy::lorawan_radio::LorawanRadio;
use lora_phy::sx127x::{self, Sx1276, Sx127x};
use lora_phy::LoRa;
use lorawan_device::async_device::{region, Device, EmbassyTimer, JoinMode};
use lorawan_device::{AppEui, AppKey, DevEui};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    RNG_LPUART1 => rng::InterruptHandler<peripherals::RNG>;
    EXTI0_1 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI0_1>;
    EXTI4_15 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4_15>;
    DMA1_CHANNEL2_3 => dma::InterruptHandler<peripherals::DMA1_CH2>, dma::InterruptHandler<peripherals::DMA1_CH3>;
});

// warning: set these appropriately for the region
const LORAWAN_REGION: region::Region = region::Region::EU868;
const MAX_TX_POWER: u8 = 14;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    config.rcc.hsi = true;
    config.rcc.sys = embassy_stm32::rcc::Sysclk::HSI;
    // The RNG needs its 48 MHz clock. Without it the clock-error flag stays
    // set and Rng::new never returns; enable HSI48 and route it to clk48.
    config.rcc.hsi48 = Some(embassy_stm32::rcc::Hsi48Config { sync_from_usb: false });
    config.rcc.mux.clk48sel = embassy_stm32::rcc::mux::Clk48sel::HSI48;
    let p = embassy_stm32::init(config);

    // Module RF plumbing (B-L072Z UM2115): PA12 powers the TCXO; the antenna
    // switch is PA1 (RX) and PC2 (TX via RFO). PC1 (TX via PA_BOOST) is unused
    // with tx_boost off, parked low. These pins must stay driven for the whole
    // program, so keep them bound.
    let _tcxo_vcc = Output::new(p.PA12, Level::High, Speed::Low);
    let _pa_boost = Output::new(p.PC1, Level::Low, Speed::Low);
    let rf_switch_rx = Output::new(p.PA1, Level::Low, Speed::Low);
    let rf_switch_tx = Output::new(p.PC2, Level::Low, Speed::Low);

    let nss = Output::new(p.PA15, Level::High, Speed::Low);
    let reset = Output::new(p.PC0, Level::High, Speed::Low);
    // The sx127x reports RxDone/TxDone on DIO0 but RxTimeout only on DIO1, and
    // LoRaWAN needs the timeout to close an empty receive window. Watch both.
    let dio0 = ExtiInput::new(p.PB4, p.EXTI4, Pull::Up, Irqs);
    let dio1 = ExtiInput::new(p.PB1, p.EXTI1, Pull::Up, Irqs);

    let mut spi_config = spi::Config::default();
    spi_config.frequency = khz(200);
    let spi = spi::Spi::new(p.SPI1, p.PB3, p.PA7, p.PA6, p.DMA1_CH3, p.DMA1_CH2, Irqs, spi_config);
    let spi = ExclusiveDevice::new(spi, nss, Delay).unwrap();

    let config = sx127x::Config {
        chip: Sx1276,
        tcxo_used: true,
        rx_boost: false,
        tx_boost: false,
    };
    let iv = GenericSx127xInterfaceVariant::new_with_secondary_irq(
        reset,
        dio0,
        Some(dio1),
        Some(rf_switch_rx),
        Some(rf_switch_tx),
    )
    .unwrap();
    let lora = LoRa::new(Sx127x::new(spi, iv, config), true, Delay).await.unwrap();

    let radio: LorawanRadio<_, _, MAX_TX_POWER> = lora.into();
    let region: region::Configuration = region::Configuration::new(LORAWAN_REGION);
    let mut device: Device<_, _, _> = Device::new(region, radio, EmbassyTimer::new(), Rng::new(p.RNG, Irqs));

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
