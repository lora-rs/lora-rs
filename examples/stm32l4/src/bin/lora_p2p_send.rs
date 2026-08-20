//! This example runs on the Semtech LR1110 development kit: an LR1110MB1DxS
//! shield on a NUCLEO-L476RG. It demonstrates LoRa P2P send functionality.
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::time::khz;
use embassy_stm32::{bind_interrupts, dma, peripherals, spi};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::LoRa;
use lora_phy::iv::GenericLr1110InterfaceVariant;
use lora_phy::lr1110::{self, Lr1110, PaSelection, SetDioAsRfSwitchParams, TcxoCtrlVoltage};
use lora_phy::mod_params::*;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    EXTI3 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI3>;
    EXTI4 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4>;
    DMA1_CHANNEL2 => dma::InterruptHandler<peripherals::DMA1_CH2>;
    DMA1_CHANNEL3 => dma::InterruptHandler<peripherals::DMA1_CH3>;
});

const LORA_FREQUENCY_IN_HZ: u32 = 903_900_000; // warning: set this appropriately for the region

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(embassy_stm32::Config::default());

    // Shield on the Arduino headers (Semtech SWSD001 pinout): NSS=D7/PA8,
    // NRESET=A0/PA0, BUSY=D3/PB3, IRQ(DIO9)=D5/PB4, SPI1 on D13/D12/D11 =
    // PA5/PA6/PA7. The RF switch is on the LR1110's own RFSW DIOs
    // (SetDioAsRfSwitch below), not on host pins.
    let nss = Output::new(p.PA8, Level::High, Speed::VeryHigh);
    let reset = Output::new(p.PA0, Level::High, Speed::Low);
    let busy = ExtiInput::new(p.PB3, p.EXTI3, Pull::None, Irqs);
    let dio9 = ExtiInput::new(p.PB4, p.EXTI4, Pull::Down, Irqs);

    let mut spi_config = spi::Config::default();
    spi_config.frequency = khz(1000);
    let spi = spi::Spi::new(p.SPI1, p.PA5, p.PA7, p.PA6, p.DMA1_CH3, p.DMA1_CH2, Irqs, spi_config);
    let spi = ExclusiveDevice::new(spi, nss, Delay).unwrap();

    // RF switch / TCXO / regulator per Semtech's shield reference
    // (SWSD001 smtc_shield_lr11x0_common.c): RFSW0+RFSW1 enabled, RX=RFSW0,
    // TX(LP)=both, TX(HP)=RFSW1; TCXO at 3.0 V; DCDC.
    let config = lr1110::Config {
        pa_selection: PaSelection::Lp,
        dio_as_rf_switch: Some(SetDioAsRfSwitchParams {
            enable: 0x03,
            standby: 0x00,
            rx: 0x01,
            tx_lp: 0x03,
            tx_hp: 0x02,
            tx_hf: 0x00,
            gnss: 0x00,
            wifi: 0x00,
        }),
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl3V0),
        use_dcdc: true,
        rx_boost: false,
    };
    let iv = GenericLr1110InterfaceVariant::new(reset, dio9, busy, None, None).unwrap();
    let mut lora = LoRa::new(Lr1110::new(spi, iv, config), false, Delay).await.unwrap();

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

    let buffer = [0x01u8, 0x02u8, 0x03u8];

    // The shield's low-power PA tops out at +14 dBm below 400 MHz.
    match lora.prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 14, &buffer).await {
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
