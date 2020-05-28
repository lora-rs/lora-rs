use hal::exti;
use hal::exti::{ExtiLine, GpioLine};
use hal::gpio::*;
use hal::pac;
use hal::prelude::*;
use hal::rcc::Rcc;
use hal::spi;

use nb::block;
use stm32l0xx_hal as hal;
use sx12xx::{AntPinsMode, BoardBindings};

type Uninitialized = Analog;

pub type RadioIRQ = gpiob::PB4<Input<PullUp>>;

pub fn initialize_irq(
    pin: gpiob::PB4<Uninitialized>,
    syscfg: &mut hal::syscfg::SYSCFG,
    exti: &mut exti::Exti,
) -> gpiob::PB4<Input<PullUp>> {
    let dio0 = pin.into_pull_up_input();

    exti.listen_gpio(
        syscfg,
        dio0.port(),
        GpioLine::from_raw_line(dio0.pin_number()).unwrap(),
        exti::TriggerEdge::Rising,
    );

    dio0
}

pub type TcxoEn = gpioa::PA8<Output<PushPull>>;

pub fn new(
    spi_peripheral: pac::SPI1,
    rcc: &mut Rcc,
    spi_sck: gpiob::PB3<Uninitialized>,
    spi_miso: gpioa::PA6<Uninitialized>,
    spi_mosi: gpioa::PA7<Uninitialized>,
    spi_nss_pin: gpioa::PA15<Uninitialized>,
    reset: gpioc::PC0<Uninitialized>,
    rx: gpioa::PA1<Uninitialized>,
    tx_rfo: gpioc::PC2<Uninitialized>,
    tx_boost: gpioc::PC1<Uninitialized>,
    tcxo_en_pin: Option<gpioa::PA8<Uninitialized>>,
) -> BoardBindings {
    let mut set_board_tcxo = None;
    // store all of the necessary pins and peripherals into statics
    // this is necessary as the extern C functions need access
    // this is safe, thanks to ownership and because these statics are private
    unsafe {
        SPI = Some(spi_peripheral.spi(
            (spi_sck, spi_miso, spi_mosi),
            spi::MODE_0,
            200_000.hz(),
            rcc,
        ));
        SPI_NSS = Some(spi_nss_pin.into_push_pull_output());
        RESET = Some(reset.into_push_pull_output());
        ANT_SW = Some(AntennaSwitches::new(
            rx.into_push_pull_output(),
            tx_rfo.into_push_pull_output(),
            tx_boost.into_push_pull_output(),
        ));
        if let Some(tcxo_en) = tcxo_en_pin {
            EN_TCXO = Some(tcxo_en.into_push_pull_output());
            set_board_tcxo = Some(set_tcxo as unsafe extern "C" fn(bool) -> u8);
        }
    };

    BoardBindings {
        reset: Some(radio_reset),
        spi_in_out: Some(spi_in_out),
        spi_nss: Some(spi_nss),
        delay_ms: Some(delay_ms),
        set_antenna_pins: Some(set_antenna_pins),
        set_board_tcxo,
        busy_pin_status: None,
        reduce_power: None,
    }
}

static mut EN_TCXO: Option<TcxoEn> = None;

#[no_mangle]
pub extern "C" fn set_tcxo(value: bool) -> u8 {
    unsafe {
        if let Some(pin) = &mut EN_TCXO {
            if value {
                pin.set_high().unwrap();
            } else {
                pin.set_low().unwrap();
            }
        }
    }
    6
}

type SpiPort = hal::spi::Spi<
    hal::pac::SPI1,
    (
        hal::gpio::gpiob::PB3<Uninitialized>,
        hal::gpio::gpioa::PA6<Uninitialized>,
        hal::gpio::gpioa::PA7<Uninitialized>,
    ),
>;
static mut SPI: Option<SpiPort> = None;
#[no_mangle]
extern "C" fn spi_in_out(out_data: u8) -> u8 {
    unsafe {
        if let Some(spi) = &mut SPI {
            spi.send(out_data).unwrap();
            block!(spi.read()).unwrap()
        } else {
            0
        }
    }
}

static mut SPI_NSS: Option<gpioa::PA15<Output<PushPull>>> = None;
#[no_mangle]
extern "C" fn spi_nss(value: bool) {
    unsafe {
        if let Some(pin) = &mut SPI_NSS {
            if value {
                pin.set_high().unwrap();
            } else {
                pin.set_low().unwrap();
            }
        }
    }
}

static mut RESET: Option<gpioc::PC0<Output<PushPull>>> = None;
#[no_mangle]
extern "C" fn radio_reset(value: bool) {
    unsafe {
        if let Some(pin) = &mut RESET {
            if value {
                pin.set_low().unwrap();
            } else {
                pin.set_high().unwrap();
            }
        }
    }
}

#[no_mangle]
extern "C" fn delay_ms(ms: u32) {
    cortex_m::asm::delay(ms);
}

pub struct AntennaSwitches<Rx, TxRfo, TxBoost> {
    rx: Rx,
    tx_rfo: TxRfo,
    tx_boost: TxBoost,
}
#[warn(unused_must_use)]
impl<Rx, TxRfo, TxBoost> AntennaSwitches<Rx, TxRfo, TxBoost>
where
    Rx: embedded_hal::digital::v2::OutputPin,
    TxRfo: embedded_hal::digital::v2::OutputPin,
    TxBoost: embedded_hal::digital::v2::OutputPin,
{
    pub fn new(rx: Rx, tx_rfo: TxRfo, tx_boost: TxBoost) -> AntennaSwitches<Rx, TxRfo, TxBoost> {
        AntennaSwitches {
            rx,
            tx_rfo,
            tx_boost,
        }
    }

    pub fn set_sleep(&mut self) {
        self.rx.set_low().unwrap_or(());
        self.tx_rfo.set_low().unwrap_or(());
        self.tx_boost.set_low().unwrap_or(());
    }

    pub fn set_tx(&mut self) {
        self.rx.set_low().unwrap_or(());
        self.tx_rfo.set_low().unwrap_or(());
        self.tx_boost.set_high().unwrap_or(());
    }

    pub fn set_rx(&mut self) {
        self.rx.set_high().unwrap_or(());
        self.tx_rfo.set_low().unwrap_or(());
        self.tx_boost.set_low().unwrap_or(());
    }
}

type AntSw = AntennaSwitches<
    stm32l0xx_hal::gpio::gpioa::PA1<stm32l0xx_hal::gpio::Output<stm32l0xx_hal::gpio::PushPull>>,
    stm32l0xx_hal::gpio::gpioc::PC2<stm32l0xx_hal::gpio::Output<stm32l0xx_hal::gpio::PushPull>>,
    stm32l0xx_hal::gpio::gpioc::PC1<stm32l0xx_hal::gpio::Output<stm32l0xx_hal::gpio::PushPull>>,
>;

static mut ANT_SW: Option<AntSw> = None;

pub extern "C" fn set_antenna_pins(mode: AntPinsMode, _power: u8) {
    unsafe {
        if let Some(ant_sw) = &mut ANT_SW {
            match mode {
                AntPinsMode::AntModeTx => {
                    ant_sw.set_tx();
                }
                AntPinsMode::AntModeRx => {
                    ant_sw.set_rx();
                }
                AntPinsMode::AntModeSleep => {
                    ant_sw.set_sleep();
                }
                _ => (),
            }
        }
    }
}
