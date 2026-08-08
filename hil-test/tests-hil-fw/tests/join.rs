//! On-target HIL tests: OTAA join against the bench gateway/harness.
//!
//! Requires the lora-hil-harness running on lounas:1730 with JoinPolicy
//! set to accept the test credentials (DevEUI/AppEUI 00..01, AppKey 00..01).
#![no_std]
#![no_main]

use defmt_rtt as _;

#[embedded_test::tests]
mod tests {
    use embassy_stm32::gpio::{Level, Output, Speed};
    use embassy_stm32::rng::{self, Rng};
    use embassy_stm32::spi::{mode::Master, Spi};
    use embassy_stm32::time::Hertz;
    use embassy_stm32::{bind_interrupts, dma, peripherals};
    use embassy_time::Delay;
    use lora_hil_fw_tests::iv::{InterruptHandler, Stm32wlInterfaceVariant, SubghzSpiDevice};
    use lora_phy::lorawan_radio::LorawanRadio;
    use lora_phy::sx126x::{self, Stm32wl, Sx126x, TcxoCtrlVoltage};
    use lora_phy::LoRa;
    use lorawan_device::async_device::{Device, EmbassyTimer, JoinMode, JoinResponse, SendResponse};
    use lorawan_device::region::{Subband, US915};
    use lorawan_device::{AppEui, AppKey, DevEui};

    const MAX_TX_POWER: u8 = 14;
    const DEVEUI: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];
    const APPEUI: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];
    const APPKEY: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

    bind_interrupts!(struct Irqs{
        SUBGHZ_RADIO => InterruptHandler;
        RNG => rng::InterruptHandler<peripherals::RNG>;
        DMA1_CHANNEL1 => dma::InterruptHandler<peripherals::DMA1_CH1>;
        DMA1_CHANNEL2 => dma::InterruptHandler<peripherals::DMA1_CH2>;
    });

    type HilDevice = Device<
        LorawanRadio<
            Sx126x<
                SubghzSpiDevice<Spi<'static, embassy_stm32::mode::Async, Master>>,
                Stm32wlInterfaceVariant<Output<'static>>,
                Stm32wl,
            >,
            Delay,
            MAX_TX_POWER,
        >,
        EmbassyTimer,
        Rng<'static, peripherals::RNG>,
    >;

    #[init]
    async fn init() -> HilDevice {
        let mut config = embassy_stm32::Config::default();
        {
            use embassy_stm32::rcc::*;
            config.rcc.hse = Some(Hse {
                freq: Hertz(32_000_000),
                mode: HseMode::Bypass,
                prescaler: HsePrescaler::DIV1,
            });
            config.rcc.sys = Sysclk::PLL1_R;
            config.rcc.pll = Some(Pll {
                source: PllSource::HSE,
                prediv: PllPreDiv::DIV2,
                mul: PllMul::MUL6,
                divp: None,
                divq: Some(PllQDiv::DIV2),
                divr: Some(PllRDiv::DIV2),
            });
        }
        let p = embassy_stm32::init(config);

        let ctrl1 = Output::new(p.PC4, Level::Low, Speed::High);
        let ctrl2 = Output::new(p.PC5, Level::Low, Speed::High);
        let ctrl3 = Output::new(p.PC3, Level::High, Speed::High);

        let spi = Spi::new_subghz(p.SUBGHZSPI, p.DMA1_CH1, p.DMA1_CH2, Irqs);
        let spi = SubghzSpiDevice(spi);
        let use_high_power_pa = true;
        let config = sx126x::Config {
            chip: Stm32wl { use_high_power_pa },
            tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
            use_dcdc: true,
            rx_boost: false,
        };
        let iv =
            Stm32wlInterfaceVariant::new(Irqs, use_high_power_pa, Some(ctrl1), Some(ctrl2), Some(ctrl3)).unwrap();
        let lora = LoRa::new(Sx126x::new(spi, iv, config), true, Delay).await.unwrap();

        let radio: LorawanRadio<_, _, MAX_TX_POWER> = lora.into();
        let mut us915 = US915::new();
        us915.set_join_bias(Subband::_2);
        Device::new(us915.into(), radio, EmbassyTimer::new(), Rng::new(p.RNG, Irqs))
    }

    /// Plumbing smoke test: no radio use; proves embedded-test + probe-rs +
    /// semihosting work on this chip before anything RF-dependent runs.
    #[test]
    #[timeout(30)]
    async fn smoke(_device: HilDevice) {
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

    /// OTAA join, allowing a few attempts: the bench antenna is damaged and
    /// RSSI swings.
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
    async fn join_ok(mut device: HilDevice) {
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
    async fn join_never_accepted(mut device: HilDevice) {
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
    async fn join_dlsettings_rx1(mut device: HilDevice) {
        join_with_retries!(device);
        expect_magic_downlink!(device);
    }

    /// PR #461 hardware validation, RX2 leg: the JoinAccept sets RX2 DR10
    /// (default DR8). The harness leaves RX1 silent and transmits in RX2 at
    /// 923.3 MHz DR10; reception proves the RX2 datarate was adopted.
    #[test]
    #[timeout(120)]
    async fn join_dlsettings_rx2(mut device: HilDevice) {
        join_with_retries!(device);
        expect_magic_downlink!(device);
    }
}
