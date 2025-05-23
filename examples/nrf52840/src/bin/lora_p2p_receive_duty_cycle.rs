//! This example runs on the RAK4631 WisBlock, which has an nRF52840 MCU and Semtech Sx126x radio.
//! Other nrf/sx126x combinations may work with appropriate pin modifications.
//! It demonstrates LoRa Rx duty cycle functionality in conjunction with the lora_p2p_send example.
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

const LORA_FREQUENCY_IN_HZ: u32 = 903_900_000; // warning: set this appropriately for the region

bind_interrupts!(struct Irqs {
    TWISPI1 => spim::InterruptHandler<peripherals::TWISPI1>;
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
    let spi = ExclusiveDevice::new(spim, nss, Delay).unwrap();

    let config = sx126x::Config {
        chip: Sx1262,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: false,
    };
    let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, Some(rf_switch_rx), Some(rf_switch_tx)).unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), false, Delay).await.unwrap();

    let mut debug_indicator = Output::new(p.P1_03, Level::Low, OutputDrive::Standard);
    let mut start_indicator = Output::new(p.P1_04, Level::Low, OutputDrive::Standard);

    start_indicator.set_high();
    Timer::after_secs(5).await;
    start_indicator.set_low();

    let mut receiving_buffer = [00u8; 100];

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

    let rx_pkt_params = {
        match lora.create_rx_packet_params(4, false, receiving_buffer.len() as u8, true, false, &mdltn_params) {
            Ok(pp) => pp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };

    // See "RM0453 Reference manual STM32WL5x advanced Arm®-based 32-bit MCUs with sub-GHz radio solution" for the best explanation of Rx duty cycle processing.
    match lora
        .prepare_for_rx(
            RxMode::DutyCycle(DutyCycleParams {
                rx_time: 300_000,    // 300_000 units * 15.625 us/unit = 4.69 s
                sleep_time: 200_000, // 200_000 units * 15.625 us/unit = 3.13 s
            }),
            &mdltn_params,
            &rx_pkt_params,
        )
        .await
    {
        Ok(()) => {}
        Err(err) => {
            info!("Radio error = {}", err);
            return;
        }
    };

    receiving_buffer = [00u8; 100];
    match lora.rx(&rx_pkt_params, &mut receiving_buffer).await {
        Ok((received_len, _rx_pkt_status)) => {
            if (received_len == 3)
                && (receiving_buffer[0] == 0x01u8)
                && (receiving_buffer[1] == 0x02u8)
                && (receiving_buffer[2] == 0x03u8)
            {
                info!("rx successful");
                debug_indicator.set_high();
                Timer::after_secs(5).await;
                debug_indicator.set_low();
            } else {
                info!("rx unknown packet")
            }
        }
        Err(err) => info!("rx unsuccessful = {}", err),
    }
}
