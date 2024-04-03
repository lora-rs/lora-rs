//! This example runs on a STM32WL board, which has a builtin Semtech Sx1262 radio.
//! It demonstrates join bias for US915 devices and Class C functionality.
#![no_std]
#![no_main]

#[path = "../iv.rs"]
mod iv;

use defmt::info;

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{AnyPin, Input, Level, Output, Pin, Pull, Speed};
use embassy_stm32::rng::{self, Rng};
use embassy_stm32::spi::Spi;
use embassy_stm32::time::Hertz;
use embassy_stm32::{bind_interrupts, peripherals};
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::Delay;

use lora_phy::lorawan_radio::LorawanRadio;
use lora_phy::sx126x::{self, Sx126x, Sx126xVariant, TcxoCtrlVoltage};
use lora_phy::LoRa;
use lorawan_device::async_device::{Device, EmbassyTimer, JoinMode, JoinResponse, SendResponse};
use lorawan_device::default_crypto::DefaultFactory as Crypto;
use lorawan_device::region::{Subband, US915};
use lorawan_device::{AppEui, AppKey, DevEui};
use {defmt_rtt as _, panic_probe as _};

use self::iv::{InterruptHandler, Stm32wlInterfaceVariant, SubghzSpiDevice};

const MAX_TX_POWER: u8 = 21;
// During uplinks, it possible to receive a class A downlink and many Class C downlinks.
// Increasing the stacks buffer to at least 3 is a good start.
const DOWNLINK_BUFFER: usize = 3;

bind_interrupts!(struct Irqs{
    SUBGHZ_RADIO => InterruptHandler;
    RNG => rng::InterruptHandler<peripherals::RNG>;
});

static CHANNEL: Channel<ThreadModeRawMutex, ButtonState, 3> = Channel::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(32_000_000),
            mode: HseMode::Bypass,
            prescaler: HsePrescaler::DIV1,
        });
        config.rcc.mux = ClockSrc::PLL1_R;
        config.rcc.pll = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV2,
            mul: PllMul::MUL6,
            divp: None,
            divq: Some(PllQDiv::DIV2), // PLL1_Q clock (32 / 2 * 6 / 2), used for RNG
            divr: Some(PllRDiv::DIV2), // sysclk 48Mhz clock (32 / 2 * 6 / 2)
        });
    }
    let p = embassy_stm32::init(config);

    // Set CTRL1 and CTRL3 for high-power transmission, while CTRL2 acts as an RF switch between tx and rx
    let _ctrl1 = Output::new(p.PC4.degrade(), Level::Low, Speed::High);
    let ctrl2 = Output::new(p.PC5.degrade(), Level::High, Speed::High);
    let _ctrl3 = Output::new(p.PC3.degrade(), Level::High, Speed::High);

    let spi = Spi::new_subghz(p.SUBGHZSPI, p.DMA1_CH1, p.DMA1_CH2);
    let spi = SubghzSpiDevice(spi);

    let config = sx126x::Config {
        chip: Sx126xVariant::Stm32wl,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        use_dio2_as_rfswitch: false,
        rx_boost: false,
    };
    let iv = Stm32wlInterfaceVariant::new(Irqs, None, Some(ctrl2)).unwrap();
    let lora = LoRa::new(Sx126x::new(spi, iv, config), true, Delay).await.unwrap();

    let _lora_task = spawner.spawn(lora_task(lora, Rng::new(p.RNG, Irqs), CHANNEL.receiver()));

    let button = Input::new(p.PA0, Pull::Up);
    let button = ExtiInput::new(button, p.EXTI0);
    let _button_task = spawner.spawn(button_task(button, CHANNEL.sender()));
}

type Stm32wlLoRa<'d> = LoRa<
    Sx126x<
        iv::SubghzSpiDevice<Spi<'d, peripherals::SUBGHZSPI, peripherals::DMA1_CH1, peripherals::DMA1_CH2>>,
        Stm32wlInterfaceVariant<Output<'d, AnyPin>>,
    >,
    Delay,
>;

#[embassy_executor::task]
async fn lora_task(
    lora: Stm32wlLoRa<'static>,
    rng: Rng<'static, peripherals::RNG>,
    rx: Receiver<'static, ThreadModeRawMutex, ButtonState, 3>,
) {
    let radio: LorawanRadio<_, _, MAX_TX_POWER> = lora.into();
    let mut us915 = US915::new();
    // Setting join bias causes the device to attempt the first join on subband 2.
    // If it fails, it will proceed with the other subbands sequentially.
    us915.set_join_bias(Subband::_2);
    let mut device: Device<_, Crypto, _, _, 256, DOWNLINK_BUFFER> =
        Device::new(us915.into(), radio, EmbassyTimer::new(), rng);
    device.enable_class_c();

    // TODO: Adjust the EUI and Keys according to your network credentials
    let join_mode = JoinMode::OTAA {
        deveui: DevEui::from([0, 0, 0, 0, 0, 0, 0, 0]),
        appeui: AppEui::from([0, 0, 0, 0, 0, 0, 0, 0]),
        appkey: AppKey::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    };

    info!("Joining LoRaWAN network");
    loop {
        let join_result = device.join(&join_mode).await;
        if let Ok(JoinResponse::JoinSuccess) = join_result {
            info!("LoRaWAN network joined");
            break;
        }
        info!("Join failed: {:?}. Retrying...", join_result);
    }

    // After joining Class C, the LoRaWAN specification indicates that it is important to send a
    // confirmed uplink immediately after joining until confirmed such that Class C downlinks are
    // enabled.
    loop {
        info!("Sending uplink...");
        let result = device.send(&[0x01, 0x02, 0x03, 0x04], 1, true).await;
        if let Ok(SendResponse::DownlinkReceived(_)) = result {
            // After an uplink with Class C enabled, it is important to check for multiple downlinks.
            // It is theoretically possible to receive a Class A downlink and any number of Class C
            // downlinks during the Class C windows.
            while let Some(downlink) = device.take_downlink() {
                info!("Received {:?}", downlink);
            }
            break;
        } else {
            info!("Uplink failed: {:?}. Retrying...", result);
        }
    }

    loop {
        let either = select(rx.receive(), device.rxc_listen()).await;
        match either {
            Either::First(button_state) => {
                info!("Button state: {:?}", button_state);
                let resp = device.send(&[0x03], 1, true).await;
                info!("Sent uplink: {:?}", resp);
                // After an uplink with Class C enabled, it is important to check for multiple downlinks.
                // It is theoretically possible to receive a Class A downlink and any number of Class C
                // downlinks during the Class C windows.
                while let Some(downlink) = device.take_downlink() {
                    info!("Received {:?}", downlink);
                }
            }
            Either::Second(downlink) => {
                info!("Received {:?}", downlink);
                while let Some(downlink) = device.take_downlink() {
                    info!("Received {:?}", downlink);
                }
            }
        }
    }
}

#[derive(defmt::Format)]
enum ButtonState {
    Pressed,
    Released,
}

#[embassy_executor::task]
async fn button_task(
    mut button: ExtiInput<'static, peripherals::PA0>,
    tx: Sender<'static, ThreadModeRawMutex, ButtonState, 3>,
) {
    info!("Press the USER button...");
    loop {
        button.wait_for_falling_edge().await;
        tx.send(ButtonState::Pressed).await;
        info!("Pressed!");
        button.wait_for_rising_edge().await;
        tx.send(ButtonState::Released).await;
        info!("Released!");
    }
}
