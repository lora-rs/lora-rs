#![cfg_attr(not(test), no_std)]
#![no_main]

// To use example, press any key in serial terminal
// Packet will send and "Transmit Done!" will print when radio is done sending packet

extern crate nb;
extern crate panic_halt;

use core::fmt::Write;
use lorawan_device::{Device as LoRaWanDevice, Event as LoRaWanEvent, Request as LoRaWanRequest};
use rtfm::app;
use stm32l0xx_hal::exti::{ExtiLine, GpioLine};
use stm32l0xx_hal::{pac, pac::Interrupt, rng, serial};
//use stm32l0xx_hal::serial::USART1 as DebugUsart;
//use stm32l0xx_hal::serial::Serial1Ext;
use stm32l0xx_hal::serial::USART2 as DebugUsart;
use stm32l0xx_hal::{exti::Exti, prelude::*, rcc, rng::Rng, syscfg, timer::Timer};
use sx12xx;
use sx12xx::Sx12xx;
mod bindings;
pub use bindings::initialize_irq as initialize_radio_irq;
pub use bindings::RadioIRQ;
pub use bindings::TcxoEn;

static mut RNG: Option<rng::Rng> = None;
fn get_random_u32() -> u32 {
    unsafe {
        if let Some(rng) = &mut RNG {
            // enable starts the ADC conversions that generate the random number
            rng.enable();
            // wait until the flag flips; interrupt driven is possible but no implemented
            rng.wait();
            // reading the result clears the ready flag
            let val = rng.take_result();
            // can save some power by disabling until next random number needed
            rng.disable();
            val
        } else {
            panic!("No Rng exists!");
        }
    }
}

pub struct TimerContext {
    pub target: u16,
    pub count: u16,
    pub enable: bool,
}

#[app(device = stm32l0xx_hal::pac, peripherals = true)]
const APP: () = {
    struct Resources {
        int: Exti,
        radio_irq: RadioIRQ,
        debug_uart: serial::Tx<DebugUsart>,
        uart_rx: serial::Rx<DebugUsart>,
        timer: Timer<pac::TIM2>,
        #[init([0;512])]
        buffer: [u8; 512],
        #[init(0)]
        count: u8,
        sx12xx: Sx12xx,
        lorawan: LoRaWanDevice<Sx12xx, sx12xx::Event>,
        #[init(TimerContext {
            target: 0,
            count: 0,
            enable: false,
        })]
        timer_context: TimerContext,
    }

    #[init(spawn = [send_ping, lorawan_event], resources = [buffer])]
    fn init(ctx: init::Context) -> init::LateResources {
        let device = ctx.device;
        let mut rcc = device.RCC.freeze(rcc::Config::hsi16());
        let mut syscfg = syscfg::SYSCFG::new(device.SYSCFG, &mut rcc);

        let gpioa = device.GPIOA.split(&mut rcc);
        let gpiob = device.GPIOB.split(&mut rcc);
        let gpioc = device.GPIOC.split(&mut rcc);

        let (tx_pin, rx_pin, serial_peripheral) = (gpioa.pa2, gpioa.pa3, device.USART2);

        let mut serial = serial_peripheral
            .usart(tx_pin, rx_pin, serial::Config::default(), &mut rcc)
            .unwrap();

        // listen for incoming bytes which will trigger transmits
        serial.listen(serial::Event::Rxne);
        let (mut tx, rx) = serial.split();

        write!(tx, "LongFi Device Test\r\n").unwrap();

        let mut exti = Exti::new(device.EXTI);

        // constructor initializes 48 MHz clock that RNG requires
        // Initialize 48 MHz clock and RNG
        let hsi48 = rcc.enable_hsi48(&mut syscfg, device.CRS);
        unsafe { RNG = Some(Rng::new(device.RNG, &mut rcc, hsi48)) };
        let radio_irq = initialize_radio_irq(gpiob.pb4, &mut syscfg, &mut exti);

        // Configure the timer.
        let timer = device.TIM2.timer(1.khz(), &mut rcc);

        let bindings = bindings::new(
            device.SPI1,
            &mut rcc,
            gpiob.pb3,
            gpioa.pa6,
            gpioa.pa7,
            gpioa.pa15,
            gpioc.pc0,
            gpioa.pa1,
            gpioc.pc2,
            gpioc.pc1,
            None, //Some(gpioa.pa8), //use pa8 for catena
        );

        let mut sx12xx = Sx12xx::new(sx12xx::Radio::sx1276(), bindings);
        sx12xx.set_public_network(true);

        let lorawan = LoRaWanDevice::new(
            [0x83, 0x19, 0x20, 0xB5, 0x5C, 0x1E, 0x16, 0x7C],
            [0x11, 0x6B, 0x8A, 0x61, 0x3E, 0x37, 0xA1, 0x0C],
            [
                0xAC, 0xC3, 0x87, 0x2A, 0x2F, 0x82, 0xED, 0x20, 0x47, 0xED, 0x18, 0x92, 0xD6, 0xFC,
                0x8C, 0x0E,
            ],
            get_random_u32,
        );

        ctx.spawn.lorawan_event(LoRaWanEvent::StartJoin).unwrap();

        write!(tx, "Going to main loop\r\n").unwrap();

        // Return the initialised resources.
        init::LateResources {
            int: exti,
            radio_irq,
            debug_uart: tx,
            uart_rx: rx,
            sx12xx,
            lorawan,
            timer,
        }
    }

    #[task(capacity = 4, priority = 2, resources = [debug_uart, buffer, sx12xx, lorawan], spawn  = [lorawan_request])]
    fn radio_event(ctx: radio_event::Context, event: sx12xx::Event) {
        let (sx12xx, lorawan) = (ctx.resources.sx12xx, ctx.resources.lorawan);
        let debug = ctx.resources.debug_uart;
        match event {
            sx12xx::Event::Sx12xxEvent_DIO0 => write!(debug, "DIO0 \r\n").unwrap(),
            _ => write!(debug, "Unexpected!\r\n").unwrap(),
        }

        if let Some(response) = lorawan.handle_radio_event(sx12xx, event) {
            if let Some(request) = response.request() {
                ctx.spawn.lorawan_request(request).unwrap();
            }
        }
    }

    #[task(capacity = 4, priority = 2, resources = [debug_uart, buffer, sx12xx, lorawan], spawn  = [lorawan_request])]
    fn lorawan_event(ctx: lorawan_event::Context, event: LoRaWanEvent) {
        let (sx12xx, lorawan) = (ctx.resources.sx12xx, ctx.resources.lorawan);
        let debug = ctx.resources.debug_uart;

        match event {
            LoRaWanEvent::TimerFired => {
                write!(debug, "Providing Timer Event!\r\n").unwrap();
            }
            _ => (),
        }

        if let Some(response) = lorawan.handle_event(sx12xx, event) {
            if let Some(request) = response.request() {
                ctx.spawn.lorawan_request(request).unwrap();
            }
        }

    }

    #[task(capacity = 4, priority = 2, resources = [debug_uart, timer_context])]
    fn lorawan_request(ctx: lorawan_request::Context, request: LoRaWanRequest) {
        match request {
            LoRaWanRequest::TimerRequest(ms) => {
                write!(ctx.resources.debug_uart, "Arming Timer {:} ms \r\n", ms).unwrap();
                // grab a lock on timer and arm a timeout
                ctx.resources.timer_context.target = ms as u16;
                ctx.resources.timer_context.count = 0;
                ctx.resources.timer_context.enable = true;
                // trigger timer so that it can set itself up
                rtfm::pend(Interrupt::TIM2);
            }
            LoRaWanRequest::Error => {
                write!(ctx.resources.debug_uart, "LoRaWanResponse::Error!!\r\n").unwrap();
            }
        }
    }

    #[task(capacity = 4, priority = 2, resources = [debug_uart, count, sx12xx, lorawan])]
    fn send_ping(ctx: send_ping::Context) {
        let (sx12xx, lorawan) = (ctx.resources.sx12xx, ctx.resources.lorawan);
        let debug = ctx.resources.debug_uart;
        write!(debug, "Sending Ping\r\n").unwrap();

        let data: [u8; 5] = [0xDE, 0xAD, 0xBE, 0xEF, *ctx.resources.count];
        *ctx.resources.count += 1;

        lorawan.send(sx12xx, &data, 1, false);
    }

    #[task(binds = USART2, priority=1, resources = [uart_rx], spawn = [send_ping])]
    fn USART2(ctx: USART2::Context) {
        // #[task(binds = USART1, priority=1, resources = [uart_rx], spawn = [send_ping])]
        // fn USART1(ctx: USART1::Context) {
        let rx = ctx.resources.uart_rx;
        rx.read().unwrap();
        ctx.spawn.send_ping().unwrap();
    }

    #[task(binds = EXTI4_15, priority = 1, resources = [radio_irq, int], spawn = [radio_event])]
    fn EXTI4_15(ctx: EXTI4_15::Context) {
        Exti::unpend(GpioLine::from_raw_line(ctx.resources.radio_irq.pin_number()).unwrap());
        ctx.spawn
            .radio_event(sx12xx::Event::Sx12xxEvent_DIO0)
            .unwrap();
    }

    // This is a pretty not scalable timeout implementation
    // but we can switch to RTFM timer queues later maybe
    #[task(binds = TIM2, resources = [timer, timer_context], spawn = [lorawan_event])]
    fn TIM2(mut ctx: TIM2::Context) {
        let timer = ctx.resources.timer;
        let spawn = ctx.spawn;
        timer.clear_irq();

        ctx.resources.timer_context.lock(|context| {
            // if timer has been disabled,
            // timeout has been dismarmed
            if !context.enable {
                context.target = 0;
                context.count = 0;
                timer.unlisten();
            } else {
                // if count is 0, we are just setting up a timeout
                if context.count == 0 {
                    timer.reset();
                    timer.listen();
                }
                context.count += 1;

                // if we have a match, timer has fired
                if context.count == context.target {
                    timer.unlisten();
                    context.count = 0;
                    context.enable = false;
                    spawn.lorawan_event(LoRaWanEvent::TimerFired).unwrap()
                }
            }
        });
    }

    // Interrupt handlers used to dispatch software tasks
    extern "C" {
        fn USART4_USART5();
    }
};
