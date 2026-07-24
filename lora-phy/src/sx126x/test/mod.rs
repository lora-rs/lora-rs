//! Compare this driver's SPI byte stream against Semtech's reference driver
//! (SWL2001, via the smtc-modem-cores bindings), and run the full LoRa
//! state machine against an emulated chip.
mod emulator;
mod fixtures;
use emulator::{get_emulated_sx1261, Chip, Mode};
use fixtures::{get_sx126x, Delayer, TestFixture};

use crate::mod_params::RxMode;
use crate::mod_traits::RadioKind;
use crate::LoRa;
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};
use smtc_modem_cores::sx126x::{Context, SleepCfg};

#[tokio::test]
async fn test_sleep_cold_start() {
    let mut smtc_sx126x = Context::new(TestFixture::new());
    smtc_sx126x.set_sleep(SleepCfg::ColdStart);
    let mut sx1261 = get_sx126x();
    sx1261.set_sleep(false, &mut Delayer).await.unwrap();
    assert_eq!(sx1261.take_spi(), smtc_sx126x.inner);
}

#[tokio::test]
async fn test_sleep_warm_start() {
    let mut smtc_sx126x = Context::new(TestFixture::new());
    smtc_sx126x.set_sleep(SleepCfg::WarmStart);
    let mut sx1261 = get_sx126x();
    sx1261.set_sleep(true, &mut Delayer).await.unwrap();
    assert_eq!(sx1261.take_spi(), smtc_sx126x.inner);
}

const TEST_FREQ_HZ: u32 = 903_900_000;

fn modulation(lora: &mut LoRa<impl RadioKind, Delayer>) -> crate::mod_params::ModulationParams {
    lora.create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, TEST_FREQ_HZ)
        .unwrap()
}

#[tokio::test]
async fn test_emulated_tx_end_to_end() {
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1261(&chip), true, Delayer).await.unwrap();

    let mdltn_params = modulation(&mut lora);
    let mut tx_pkt_params = lora
        .create_tx_packet_params(8, false, true, false, &mdltn_params)
        .unwrap();

    let payload = b"hello lora";
    lora.prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 10, payload)
        .await
        .unwrap();
    lora.tx().await.unwrap();

    chip.with_model(|m| {
        assert_eq!(m.tx_log.len(), 1);
        assert_eq!(m.tx_log[0].payload, payload);
        assert_eq!(m.mode, Mode::Standby);

        // Frequency round-trips through the chip's PLL-step encoding
        let hz = (m.tx_log[0].frequency_raw as u64 * 32_000_000) >> 25;
        assert!((hz as i64 - TEST_FREQ_HZ as i64).abs() <= 1, "freq {hz}");

        // LoRa public network sync word 0x34 lands in register 0x0740 as 0x34 0x44
        assert_eq!(m.registers[&0x0740], 0x34);
        assert_eq!(m.registers[&0x0741], 0x44);
    });
}

#[tokio::test]
async fn test_emulated_rx_end_to_end() {
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1261(&chip), true, Delayer).await.unwrap();

    let mdltn_params = modulation(&mut lora);
    let rx_pkt_params = lora
        .create_rx_packet_params(8, false, 255, true, false, &mdltn_params)
        .unwrap();
    lora.prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
        .await
        .unwrap();

    chip.inject_rx(b"ping", -80, 5);

    let mut buf = [0u8; 255];
    let (len, status) = lora.rx(&rx_pkt_params, &mut buf).await.unwrap();
    assert_eq!(&buf[..len as usize], b"ping");
    assert_eq!(status.rssi, -80);
    assert_eq!(status.snr, 5);
}

#[tokio::test]
async fn test_emulated_sleep_and_wake() {
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1261(&chip), true, Delayer).await.unwrap();

    lora.sleep(true).await.unwrap();
    chip.with_model(|m| assert_eq!(m.mode, Mode::Sleep));

    // Wake back up into a working tx
    let mdltn_params = modulation(&mut lora);
    let mut tx_pkt_params = lora
        .create_tx_packet_params(8, false, true, false, &mdltn_params)
        .unwrap();
    lora.prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 10, b"wake")
        .await
        .unwrap();
    lora.tx().await.unwrap();
    chip.with_model(|m| {
        assert_eq!(m.tx_log.len(), 1);
        assert_eq!(m.tx_log[0].payload, b"wake");
    });
}
