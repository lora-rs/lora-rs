//! This example runs on the RAK4631 WisBlock, which has an nRF52840 MCU and Semtech Sx126x radio.
//! Other nrf/sx126x combinations may work with appropriate pin modifications.
//! It demonstrates LORA get instant RSSI while in receive/listen mode.
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pin as _, Pull};
use embassy_nrf::{bind_interrupts, peripherals, spim};
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx126xInterfaceVariant;
use lora_phy::sx126x::{Sx1262, Sx126x, TcxoCtrlVoltage};
use lora_phy::{mod_params::*, sx126x};
use lora_phy::{LoRa, RxMode};
use {defmt_rtt as _, panic_probe as _};

const LORA_FREQUENCY_IN_HZ: u32 = 869_400_000; // warning: set this appropriately for the region
const RSSI_THRESHOLD: i16 = -100;

bind_interrupts!(struct Irqs {
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => spim::InterruptHandler<peripherals::TWISPI1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    let nss = Output::new(p.P1_10.degrade(), Level::High, OutputDrive::Standard);
    let reset = Output::new(p.P1_06.degrade(), Level::High, OutputDrive::Standard);
    let dio1 = Input::new(p.P1_15.degrade(), Pull::Down);
    let busy = Input::new(p.P1_14.degrade(), Pull::None);
    let rf_switch_rx = Output::new(p.P1_05.degrade(), Level::Low, OutputDrive::Standard);
    let rf_switch_tx = Output::new(p.P1_07.degrade(), Level::Low, OutputDrive::Standard);

    let mut spi_config = spim::Config::default();
    spi_config.frequency = spim::Frequency::M16;
    let spim = spim::Spim::new(p.TWISPI1, Irqs, p.P1_11, p.P1_13, p.P1_12, spi_config);
    let spi = ExclusiveDevice::new(spim, nss, Delay);

    let config = sx126x::Config {
        chip: Sx1262,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: false,
    };
    let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, None, None).unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), false, Delay).await.unwrap();

    let mut rssi_above_threshold = Output::new(p.P1_03, Level::Low, OutputDrive::Standard);
    let mut rssi_below_threshold = Output::new(p.P1_04, Level::Low, OutputDrive::Standard);

    match lora.listen(LORA_FREQUENCY_IN_HZ, Bandwidth::_500KHz).await {
        Ok(()) => {
            info!("Listen")
        }
        Err(err) => {
            info!("Radio error = {}", err);
            return;
        }
    };

    let mut counter = 0;

    loop {
        let rssi = match lora.get_rssi().await {
            Ok(rssi) => {
                info!("RSSI: {}", rssi);
                rssi
            }
            Err(err) => {
                info!("Radio error = {}", err);
                -128
            }
        };
        if rssi > RSSI_THRESHOLD {
            rssi_above_threshold.set_high();
            Timer::after_millis(100).await;
            rssi_above_threshold.set_low();
            Timer::after_millis(100).await;
        } else {
            rssi_below_threshold.set_high();
            Timer::after_millis(100).await;
            rssi_below_threshold.set_low();
            Timer::after_millis(100).await;
        }
    }
}
