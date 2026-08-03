//! On-target HIL tests: OTAA join against the bench gateway/harness.
//!
//! Requires the lora-hil-harness running on lounas:1730 with JoinPolicy
//! set to accept the test credentials (DevEUI/AppEUI 00..01, AppKey 00..01).
#![no_std]
#![no_main]

use defmt_rtt as _;

#[embedded_test::tests]
mod tests {
    use embassy_stm32::gpio::{Level, Output, Pin, Speed};
    use embassy_stm32::rng::{self, Rng};
    use embassy_stm32::spi::Spi;
    use embassy_stm32::time::Hertz;
    use embassy_stm32::{bind_interrupts, peripherals};
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
    });

    type HilDevice = Device<
        LorawanRadio<
            Sx126x<
                SubghzSpiDevice<Spi<'static, embassy_stm32::mode::Async>>,
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

        let ctrl1 = Output::new(p.PC4.degrade(), Level::Low, Speed::High);
        let ctrl2 = Output::new(p.PC5.degrade(), Level::Low, Speed::High);
        let ctrl3 = Output::new(p.PC3.degrade(), Level::High, Speed::High);

        let spi = Spi::new_subghz(p.SUBGHZSPI, p.DMA1_CH1, p.DMA1_CH2);
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

    /// Full OTAA join against the harness, then one confirmed-path uplink.
    /// Allows a few attempts: the bench antenna is damaged and RSSI swings.
    #[test]
    #[timeout(120)]
    async fn join_ok(mut device: HilDevice) {
        let join_mode = JoinMode::OTAA {
            deveui: DevEui::from(DEVEUI),
            appeui: AppEui::from(APPEUI),
            appkey: AppKey::from(APPKEY),
        };

        let mut joined = false;
        for attempt in 0..5u32 {
            match device.join(&join_mode).await {
                Ok(JoinResponse::JoinSuccess) => {
                    defmt::info!("joined on attempt {}", attempt);
                    joined = true;
                    break;
                }
                Ok(JoinResponse::NoJoinAccept) => defmt::warn!("attempt {}: no JoinAccept", attempt),
                Err(e) => defmt::warn!("attempt {}: join error {:?}", attempt, e),
            }
            embassy_time::Timer::after_secs(5).await;
        }
        assert!(joined, "no successful join in 5 attempts");

        // One post-join uplink so the harness can verify session keys.
        let sent = device.send(&[0xC0, 0xFF, 0xEE, 0x01], 1, false).await;
        assert!(
            matches!(sent, Ok(SendResponse::RxComplete | SendResponse::NoAck)),
            "post-join uplink failed"
        );
    }
}
