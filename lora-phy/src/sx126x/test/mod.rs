//! Compare this driver's SPI byte stream against Semtech's reference driver
//! (SWL2001, via the smtc-modem-cores bindings), and run the full LoRa
//! state machine against an emulated chip.
mod emulator;
mod fixtures;
use emulator::{get_emulated_sx1261, Chip, Mode};
use fixtures::{get_sx126x, Delayer, TestFixture};

use crate::mod_params::{RadioMode, RxMode};
use crate::mod_traits::RadioKind;
use crate::LoRa;
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};
use smtc_modem_cores::sx126x::{
    sx126x_cad_exit_modes_e, sx126x_cad_params_t, sx126x_cad_symbs_e, sx126x_lora_bw_e, sx126x_lora_cr_e,
    sx126x_lora_pkt_len_modes_e, sx126x_lora_sf_e, sx126x_mod_params_lora_t, sx126x_pkt_params_lora_t,
    sx126x_standby_cfgs_e, Context, SleepCfg,
};

fn reference() -> Context<TestFixture> {
    Context::new(TestFixture::new())
}

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

#[tokio::test]
async fn test_standby() {
    let mut reference = reference();
    reference.set_standby(sx126x_standby_cfgs_e::SX126X_STANDBY_CFG_RC);
    let mut sx1261 = get_sx126x();
    sx1261.set_standby().await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_set_channel() {
    for freq in [433_000_000u32, 868_100_000, TEST_FREQ_HZ] {
        let mut reference = reference();
        reference.set_rf_freq(freq);
        let mut sx1261 = get_sx126x();
        sx1261.set_channel(freq).await.unwrap();
        assert_eq!(sx1261.take_spi(), reference.inner, "freq {freq}");
    }
}

#[tokio::test]
async fn test_modulation_params() {
    // Second case exercises the 500 kHz TxModulation workaround branch and
    // the low-data-rate-optimize flag
    let cases = [
        (
            SpreadingFactor::_7,
            Bandwidth::_125KHz,
            sx126x_lora_sf_e::SX126X_LORA_SF7,
            sx126x_lora_bw_e::SX126X_LORA_BW_125,
            0u8,
        ),
        (
            SpreadingFactor::_12,
            Bandwidth::_500KHz,
            sx126x_lora_sf_e::SX126X_LORA_SF12,
            sx126x_lora_bw_e::SX126X_LORA_BW_500,
            0u8,
        ),
        (
            SpreadingFactor::_11,
            Bandwidth::_125KHz,
            sx126x_lora_sf_e::SX126X_LORA_SF11,
            sx126x_lora_bw_e::SX126X_LORA_BW_125,
            1u8,
        ),
    ];
    for (sf, bw, c_sf, c_bw, ldro) in cases {
        let mut reference = reference();
        reference.set_lora_mod_params(&sx126x_mod_params_lora_t {
            sf: c_sf,
            bw: c_bw,
            cr: sx126x_lora_cr_e::SX126X_LORA_CR_4_5,
            ldro,
        });
        let mut sx1261 = get_sx126x();
        let mdltn_params = sx1261
            .create_modulation_params(sf, bw, CodingRate::_4_5, TEST_FREQ_HZ)
            .unwrap();
        assert_eq!(mdltn_params.low_data_rate_optimize, ldro, "ldro for {sf:?}/{bw:?}");
        sx1261.set_modulation_params(&mdltn_params).await.unwrap();
        assert_eq!(sx1261.take_spi(), reference.inner, "{sf:?}/{bw:?}");
    }
}

#[tokio::test]
async fn test_packet_params() {
    for iq_inverted in [false, true] {
        let mut reference = reference();
        reference.set_lora_pkt_params(&sx126x_pkt_params_lora_t {
            preamble_len_in_symb: 8,
            header_type: sx126x_lora_pkt_len_modes_e::SX126X_LORA_PKT_EXPLICIT,
            pld_len_in_bytes: 32,
            crc_is_on: true,
            invert_iq_is_on: iq_inverted,
        });
        let mut sx1261 = get_sx126x();
        let mdltn_params = sx1261
            .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, TEST_FREQ_HZ)
            .unwrap();
        let pkt_params = sx1261
            .create_packet_params(8, false, 32, true, iq_inverted, &mdltn_params)
            .unwrap();
        sx1261.set_packet_params(&pkt_params).await.unwrap();
        assert_eq!(sx1261.take_spi(), reference.inner, "iq_inverted {iq_inverted}");
    }
}

#[tokio::test]
async fn test_sync_word() {
    // Our driver skips the read-modify-write and writes the known reset
    // values (0x14 0x24) with the sync nibbles applied; prime the reference
    // with those reset values and both land on the same register write.
    let mut fixture = TestFixture::new();
    fixture.prime_read(&[0x1D, 0x07, 0x40, 0x00], &[0x14, 0x24]);
    let mut reference = Context::new(fixture);
    reference.set_lora_sync_word(0x34);
    match reference.inner.writes()[..] {
        [fixtures::Ops::Write(bytes)] => assert_eq!(bytes, &[0x0D, 0x07, 0x40, 0x34, 0x44]),
        ref other => panic!("unexpected write stream {other:?}"),
    }
    // 0x34 -> [0x34, 0x44] is asserted against our driver in
    // sx126x::tests::test_convert_sync_word
}

#[tokio::test]
async fn test_buffer_base_address() {
    let mut reference = reference();
    reference.set_buffer_base_address(0, 0);
    let mut sx1261 = get_sx126x();
    sx1261.set_tx_rx_buffer_base_address(0, 0).await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_write_buffer() {
    let payload = b"hello reference driver";
    let mut reference = reference();
    reference.write_buffer(0, payload);
    let mut sx1261 = get_sx126x();
    sx1261.set_payload(payload).await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_set_tx() {
    let mut reference = reference();
    reference.set_tx(0);
    let mut sx1261 = get_sx126x();
    sx1261.do_tx().await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_dio_irq_params() {
    // Transmit mode masks: TxDone | RxTxTimeout
    let mask = 0x0201;
    let mut reference = reference();
    reference.set_dio_irq_params(mask, mask, 0, 0);
    let mut sx1261 = get_sx126x();
    sx1261.set_irq_params(Some(RadioMode::Transmit)).await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_clear_irq_status() {
    let mut reference = reference();
    reference.clear_irq_status(0xFFFF);
    let mut sx1261 = get_sx126x();
    sx1261.clear_irq_status().await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_do_rx() {
    // (mode, symbol timeout the driver programs, SetRx timeout in rtc steps)
    let cases = [
        ("continuous", RxMode::Continuous, 0u8, 0xFFFFFF_u32),
        ("single8", RxMode::Single(8), 8, 0),
    ];
    for (label, mode, symbs, rtc_timeout) in cases {
        let mut reference = reference();
        reference.stop_timer_on_preamble(true);
        reference.set_lora_symb_nb_timeout(symbs);
        reference.cfg_rx_boosted(true);
        reference.set_rx_with_timeout_in_rtc_step(rtc_timeout);

        let mut sx1261 = get_sx126x();
        sx1261.do_rx(mode).await.unwrap();
        assert_eq!(sx1261.take_spi(), reference.inner, "{label}");
    }
}

#[tokio::test]
async fn test_do_cad() {
    let mut reference = reference();
    reference.cfg_rx_boosted(true);
    reference.set_cad_params(&sx126x_cad_params_t {
        cad_symb_nb: sx126x_cad_symbs_e::SX126X_CAD_08_SYMB,
        cad_detect_peak: 7 + 13, // SF7 + 13, per Semtech's CAD application note
        cad_detect_min: 10,
        cad_exit_mode: sx126x_cad_exit_modes_e::SX126X_CAD_ONLY,
        cad_timeout: 0,
    });
    reference.set_cad();

    let mut sx1261 = get_sx126x();
    let mdltn_params = sx1261
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, TEST_FREQ_HZ)
        .unwrap();
    sx1261.do_cad(&mdltn_params).await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
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
