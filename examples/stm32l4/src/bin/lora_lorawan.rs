//! This example runs on the Semtech LR1110 development kit: an LR1110MB1DxS
//! shield on a NUCLEO-L476RG. It demonstrates LoRaWAN join functionality.
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
use lora_phy::LoRa;
use lora_phy::iv::GenericLr1110InterfaceVariant;
use lora_phy::lorawan_radio::LorawanRadio;
use lora_phy::lr1110::{self, Lr1110, PaSelection, SetDioAsRfSwitchParams, TcxoCtrlVoltage};
use lorawan_device::async_device::{Device, EmbassyTimer, JoinMode, region};
use lorawan_device::{AppEui, AppKey, DevEui};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    RNG => rng::InterruptHandler<peripherals::RNG>;
    EXTI3 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI3>;
    EXTI4 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4>;
    DMA1_CHANNEL2 => dma::InterruptHandler<peripherals::DMA1_CH2>;
    DMA1_CHANNEL3 => dma::InterruptHandler<peripherals::DMA1_CH3>;
});

// warning: set these appropriately for the region
const LORAWAN_REGION: region::Region = region::Region::US915;
// The shield's low-power PA tops out at +14 dBm below 400 MHz.
const MAX_TX_POWER: u8 = 14;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    // The RNG needs a valid 48 MHz clock or its clock-error flag never
    // clears. The L476 has no HSI48, so take both sysclk and CLK48 from the
    // main PLL fed by MSI at 4 MHz: VCO 96 MHz, /2 on both taps. (Raising
    // MSI itself to 48 MHz as sysclk trips an embassy-stm32 0.6 init
    // ordering bug: the MSI range is raised before flash wait states are
    // set; fixed upstream after 0.6.)
    {
        use embassy_stm32::rcc::{MSIRange, Pll, PllMul, PllPreDiv, PllQDiv, PllRDiv, PllSource, Sysclk};
        config.rcc.msi = Some(MSIRange::RANGE4M);
        config.rcc.pll = Some(Pll {
            source: PllSource::MSI,
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL24,
            divp: None,
            divq: Some(PllQDiv::DIV2), // 48 MHz -> CLK48 (RNG)
            divr: Some(PllRDiv::DIV2), // 48 MHz sysclk
        });
        config.rcc.sys = Sysclk::PLL1_R;
        config.rcc.mux.clk48sel = embassy_stm32::rcc::mux::Clk48sel::PLL1_Q;
    }
    let p = embassy_stm32::init(config);

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
    let lora = LoRa::new(Lr1110::new(spi, iv, config), true, Delay).await.unwrap();

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
