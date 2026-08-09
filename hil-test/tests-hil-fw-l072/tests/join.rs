//! On-target HIL tests: OTAA join against the bench gateway/harness, on the
//! B-L072Z-LRWAN1 DUT (SX1276 via the Murata CMWX1ZZABZ module).
//!
//! Same test set as ../../tests-hil-fw/tests/join.rs (the WL55 DUT); the two
//! change together. Requires the tests-hil fixture running on lounas:1730
//! with JoinPolicy set to accept the test credentials (AppKey 00..01; this
//! board joins as DevEUI 00..02 so bench captures can tell the DUTs apart).
#![no_std]
#![no_main]

use defmt_rtt as _;

#[embedded_test::tests]
mod tests {
    use embassy_embedded_hal::adapter::BlockingAsync;
    use embassy_stm32::exti::{self, ExtiInput};
    use embassy_stm32::gpio::{Level, Output, Pull, Speed};
    use embassy_stm32::rng::{self, Rng};
    use embassy_stm32::spi::{mode::Master, Spi};
    use embassy_stm32::time::khz;
    use embassy_stm32::{bind_interrupts, peripherals};
    use embassy_time::Delay;
    use embedded_hal_bus::spi::ExclusiveDevice;
    use lora_phy::iv::GenericSx127xInterfaceVariant;
    use lora_phy::lorawan_radio::LorawanRadio;
    use lora_phy::sx127x::{self, Sx1276, Sx127x};
    use lora_phy::LoRa;
    use lorawan_device::async_device::{Device, EmbassyTimer, JoinMode, JoinResponse, SendResponse};
    use lorawan_device::region::{Subband, US915};
    use lorawan_device::{AppEui, AppKey, DevEui};

    // RFO path (tx_boost false) tops out at +14 dBm on the SX1276.
    const MAX_TX_POWER: u8 = 14;
    const DEVEUI: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 2];
    const APPEUI: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];
    const APPKEY: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

    bind_interrupts!(struct Irqs {
        RNG_LPUART1 => rng::InterruptHandler<peripherals::RNG>;
        EXTI0_1 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI0_1>;
        EXTI4_15 => exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4_15>;
    });

    type HilDevice = Device<
        LorawanRadio<
            Sx127x<
                ExclusiveDevice<
                    BlockingAsync<Spi<'static, embassy_stm32::mode::Blocking, Master>>,
                    Output<'static>,
                    Delay,
                >,
                GenericSx127xInterfaceVariant<Output<'static>, ExtiInput<'static, embassy_stm32::mode::Async>>,
                Sx1276,
            >,
            Delay,
            MAX_TX_POWER,
        >,
        EmbassyTimer,
        Rng<'static, peripherals::RNG>,
    >;

    /// The device lives in a StaticCell, not in the test future: embedded-test
    /// materializes one static task pool per test, and with the ~1 KB Device
    /// (plus its init-time copies) inlined in every future the five pools eat
    /// so much of the 20 KB RAM that the stack overflows into .bss during RF
    /// operations, corrupting waker statics. Safe to init unconditionally:
    /// probe-rs resets the core for every test. Wrapped in a newtype because
    /// embedded-test rejects bare reference parameters.
    pub struct Dev(&'static mut HilDevice);

    #[init]
    async fn init() -> Dev {
        let mut config = embassy_stm32::Config::default();
        config.rcc.hsi = true;
        config.rcc.sys = embassy_stm32::rcc::Sysclk::HSI;
        // The RNG's 48 MHz clock. Without it the RNG holds a permanent
        // clock-error flag and embassy 0.6's next_u32<->reset error
        // recovery recurses until RAM runs out.
        config.rcc.hsi48 = Some(embassy_stm32::rcc::Hsi48Config { sync_from_usb: false });
        config.rcc.mux.clk48sel = embassy_stm32::rcc::mux::Clk48sel::HSI48;
        let p = embassy_stm32::init(config);

        // Module RF plumbing (B-L072Z UM2115): PA12 powers the TCXO; the
        // antenna switch is PA1 (RX), PC2 (TX via RFO), PC1 (TX via
        // PA_BOOST, unused with tx_boost off — parked low). PA12/PC1 must
        // outlive init: dropping an embassy Output disconnects the pin.
        //
        // DIO0 (PB4) and DIO1 (PB1) both feed the interface variant: the
        // sx127x routes RxTimeout only to DIO1, so an RX window that hears
        // nothing needs DIO1 to wake the driver (lora-rs #476 / #312).
        let tcxo_vcc = Output::new(p.PA12, Level::High, Speed::Low);
        let boost_off = Output::new(p.PC1, Level::Low, Speed::Low);
        core::mem::forget(tcxo_vcc);
        core::mem::forget(boost_off);
        let rf_switch_rx = Output::new(p.PA1, Level::Low, Speed::Low);
        let rf_switch_tx = Output::new(p.PC2, Level::Low, Speed::Low);
        embassy_time::Timer::after_millis(5).await; // TCXO settle

        let nss = Output::new(p.PA15, Level::High, Speed::Low);
        let reset = Output::new(p.PC0, Level::High, Speed::Low);
        let dio0 = ExtiInput::new(p.PB4, p.EXTI4, Pull::Up, Irqs);
        let dio1 = ExtiInput::new(p.PB1, p.EXTI1, Pull::Up, Irqs);

        let mut spi_config = embassy_stm32::spi::Config::default();
        spi_config.frequency = khz(200);
        // Blocking SPI on purpose: with the DMA path (DMA1_CH2/CH3), the
        // DMA IRQ HardFaults in AtomicWaker::wake mid-join on this chip
        // (embassy-stm32 0.6, thumbv6m). Transfers are tiny and the bus
        // runs at 200 kHz; polling costs nothing the radio would notice.
        let spi = BlockingAsync::new(Spi::new_blocking(p.SPI1, p.PB3, p.PA7, p.PA6, spi_config));
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
        let mut us915 = US915::new();
        us915.set_join_bias(Subband::_2);
        static DEVICE: static_cell::StaticCell<HilDevice> = static_cell::StaticCell::new();
        Dev(DEVICE.init(Device::new(us915.into(), radio, EmbassyTimer::new(), Rng::new(p.RNG, Irqs))))
    }

    /// Plumbing smoke test: no radio use; proves embedded-test + probe-rs +
    /// semihosting work on this chip before anything RF-dependent runs.
    #[test]
    #[timeout(30)]
    async fn smoke(_device: Dev) {
        assert_eq!(2 + 2, 4);
    }

    /// Downlink the harness sends in the DLSettings scenarios (fixture keeps
    /// these in tests/scenarios.rs; change them together).
    const MAGIC_DOWNLINK: &[u8] = &[0xBE, 0xEF, 0x42];
    const MAGIC_FPORT: u8 = 42;

    // embedded-test's macro rejects plain helper fns inside this mod, so the
    // shared steps are macros.
    macro_rules! join_mode {
        () => {
            JoinMode::OTAA {
                deveui: DevEui::from(DEVEUI),
                appeui: AppEui::from(APPEUI),
                appkey: AppKey::from(APPKEY),
            }
        };
    }

    /// OTAA join, allowing a few attempts: this DUT runs a whip antenna
    /// (coax connectors pending), so link quality is uncontrolled.
    macro_rules! join_with_retries {
        ($device:expr) => {{
            let join_mode = join_mode!();
            let mut joined = false;
            for attempt in 0..5u32 {
                match $device.join(&join_mode).await {
                    Ok(JoinResponse::JoinSuccess) => {
                        defmt::info!("joined on attempt {}", attempt);
                        joined = true;
                        break;
                    }
                    Ok(JoinResponse::NoJoinAccept) => {
                        defmt::warn!("attempt {}: no JoinAccept", attempt)
                    }
                    Err(e) => defmt::warn!("attempt {}: join error {:?}", attempt, e),
                }
                embassy_time::Timer::after_secs(5).await;
            }
            assert!(joined, "no successful join in 5 attempts");
        }};
    }

    /// Send heartbeats until the harness's reply downlink arrives, then check
    /// its FPort and payload. Reception is the proof that the device is
    /// listening where the JoinAccept's DLSettings/RxDelay told it to.
    macro_rules! expect_magic_downlink {
        ($device:expr) => {{
            let mut downlink = None;
            for i in 0..4u32 {
                match $device.send(&[0xC0, 0xFF, 0xEE, 0x02], 1, false).await {
                    Ok(SendResponse::DownlinkReceived(fcnt)) => {
                        defmt::info!("downlink received on send {} (FCntDown {})", i, fcnt);
                        downlink = $device.take_downlink();
                        break;
                    }
                    Ok(r) => defmt::warn!("send {}: no downlink ({:?})", i, r),
                    Err(e) => defmt::warn!("send {}: error {:?}", i, e),
                }
                embassy_time::Timer::after_secs(2).await;
            }
            match downlink {
                Some(dl) => {
                    assert_eq!(dl.fport, MAGIC_FPORT);
                    assert_eq!(&dl.data[..], MAGIC_DOWNLINK);
                }
                None => assert!(false, "no downlink received in 4 uplinks"),
            }
        }};
    }

    /// Full OTAA join against the harness, then one confirmed-path uplink.
    #[test]
    #[timeout(120)]
    async fn join_ok(device: Dev) {
        let device = device.0;
        join_with_retries!(device);

        // One post-join uplink so the harness can verify session keys.
        let sent = device.send(&[0xC0, 0xFF, 0xEE, 0x01], 1, false).await;
        assert!(
            matches!(sent, Ok(SendResponse::RxComplete | SendResponse::NoAck)),
            "post-join uplink failed"
        );
    }

    /// Negative: paired with a harness that ignores JoinRequests or answers
    /// with a MIC-tampered JoinAccept. Passes only if no attempt ever joins.
    /// The network-side fixture separately asserts no heartbeat decrypts.
    #[test]
    #[timeout(90)]
    async fn join_never_accepted(device: Dev) {
        let device = device.0;
        let join_mode = join_mode!();
        for attempt in 0..3u32 {
            match device.join(&join_mode).await {
                Ok(JoinResponse::JoinSuccess) => {
                    defmt::error!("attempt {}: join unexpectedly succeeded", attempt);
                    assert!(false, "device joined; it must reject/miss this accept");
                }
                Ok(JoinResponse::NoJoinAccept) => {
                    defmt::info!("attempt {}: no JoinAccept, as expected", attempt)
                }
                Err(e) => defmt::warn!("attempt {}: join error {:?}", attempt, e),
            }
            embassy_time::Timer::after_secs(2).await;
        }
    }

    /// PR #461 hardware validation, RX1 leg: the JoinAccept carries
    /// RX1DROffset 2 and RxDelay 3, so the harness's reply downlink comes
    /// 3 s after the uplink at DR8 (SF12/500) instead of the default 1 s at
    /// DR10 (SF10/500). A device that ignores DLSettings misses it.
    #[test]
    #[timeout(120)]
    async fn join_dlsettings_rx1(device: Dev) {
        let device = device.0;
        join_with_retries!(device);
        expect_magic_downlink!(device);
    }

    /// PR #461 hardware validation, RX2 leg: the JoinAccept sets RX2 DR10
    /// (default DR8). The harness leaves RX1 silent and transmits in RX2 at
    /// 923.3 MHz DR10; reception proves the RX2 datarate was adopted.
    #[test]
    #[timeout(120)]
    async fn join_dlsettings_rx2(device: Dev) {
        let device = device.0;
        join_with_retries!(device);
        expect_magic_downlink!(device);
    }
}
