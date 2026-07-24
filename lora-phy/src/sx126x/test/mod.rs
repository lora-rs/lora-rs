//! Compare this driver's SPI byte stream against Semtech's reference driver
//! (SWL2001, via the smtc-modem-cores bindings), and run the full LoRa
//! state machine against an emulated chip.
mod emulator;
mod fixtures;
use emulator::{get_emulated_sx1261, Chip, Mode};
use fixtures::{get_sx1262, get_sx126x, Delayer, TestFixture};

use crate::mod_params::{RadioMode, RxMode};
use crate::mod_traits::RadioKind;
use crate::LoRa;
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};
use smtc_modem_cores::sx126x::{
    sx126x_cad_exit_modes_e, sx126x_cad_params_t, sx126x_cad_symbs_e, sx126x_lora_bw_e, sx126x_lora_cr_e,
    sx126x_lora_pkt_len_modes_e, sx126x_lora_sf_e, sx126x_mod_params_lora_t, sx126x_pa_cfg_params_t,
    sx126x_pkt_params_lora_t, sx126x_pkt_types_e, sx126x_ramp_time_e, sx126x_standby_cfgs_e, Context, SleepCfg,
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

#[tokio::test]
async fn test_get_rx_payload() {
    // Chip reports 4 bytes at offset 0. Ours reads a status byte our
    // reference discards via a NOP write; wire streams are canonically equal.
    let mut sx1261 = get_sx126x();
    sx1261.spi_mut().prime_read(&[0x13], &[0x00, 4, 0]);
    let mdltn_params = sx1261
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, TEST_FREQ_HZ)
        .unwrap();
    let pkt_params = sx1261
        .create_packet_params(8, false, 255, true, false, &mdltn_params)
        .unwrap();
    let mut buf = [0u8; 255];
    let len = sx1261.get_rx_payload(&pkt_params, &mut buf).await.unwrap();
    assert_eq!(len, 4);

    let mut reference = reference();
    let (_, rx_status) = reference.get_rx_buffer_status();
    let mut c_buf = [0u8; 4];
    reference.read_buffer(rx_status.buffer_start_pointer, &mut c_buf);
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_get_packet_status() {
    // Raw chip bytes: rssi 160, snr 20, signal rssi 162
    let mut sx1261 = get_sx126x();
    sx1261.spi_mut().prime_read(&[0x14], &[0x00, 160, 20, 162]);
    let pkt_status = sx1261.get_rx_packet_status().await.unwrap();

    let mut fixture = TestFixture::new();
    fixture.prime_read(&[0x14, 0x00], &[160, 20, 162]);
    let mut reference = Context::new(fixture);
    let (_, c_status) = reference.get_lora_pkt_status();

    // Byte streams and the decoded values both match the reference
    assert_eq!(sx1261.take_spi(), reference.inner);
    assert_eq!(pkt_status.rssi, c_status.rssi_pkt_in_dbm as i16);
    assert_eq!(pkt_status.snr, c_status.snr_pkt_in_db as i16);
}

#[tokio::test]
async fn test_get_rssi() {
    let mut sx1261 = get_sx126x();
    sx1261.spi_mut().prime_read(&[0x15], &[0x00, 161]);
    let rssi = sx1261.get_rssi().await.unwrap();

    let mut fixture = TestFixture::new();
    fixture.prime_read(&[0x15, 0x00], &[161]);
    let mut reference = Context::new(fixture);
    let (_, c_rssi) = reference.get_rssi_inst();

    assert_eq!(sx1261.take_spi(), reference.inner);
    assert_eq!(rssi, c_rssi);
}

#[tokio::test]
async fn test_get_irq_status() {
    use crate::mod_traits::IrqState;
    let mut sx1261 = get_sx126x();
    sx1261.spi_mut().prime_read(&[0x12], &[0x00, 0x00, 0x01]); // TxDone
    let state = sx1261
        .process_irq_event(RadioMode::Transmit, None, false)
        .await
        .unwrap();
    assert!(matches!(state, Some(IrqState::Done)));

    let mut fixture = TestFixture::new();
    fixture.prime_read(&[0x12, 0x00], &[0x00, 0x01]);
    let mut reference = Context::new(fixture);
    let (_, c_irq) = reference.get_irq_status();

    assert_eq!(sx1261.take_spi(), reference.inner);
    assert_eq!(c_irq, 0x0001);
}

#[tokio::test]
async fn test_ensure_ready_after_sleep() {
    // Waking from sleep: ours writes GetStatus + NOP, the reference reads one
    // status byte after GetStatus — same bytes on the wire
    let mut sx1261 = get_sx126x();
    sx1261.ensure_ready(RadioMode::Sleep).await.unwrap();

    let mut reference = reference();
    reference.get_status();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_calibrate_image() {
    // Our band table follows datasheet Table 9-2. The reference's Hz helper
    // (cal_img_in_mhz) computes ceil(band_end/4) instead, which lands one
    // step short of the table for most bands (e.g. 928 MHz -> 0xE8 vs 0xE9),
    // so compare against cal_img with the table bytes.
    let cases = [
        (903_900_000u32, 0xE1u8, 0xE9u8),
        (868_100_000, 0xD7, 0xDB),
        (433_000_000, 0x6B, 0x6F),
    ];
    for (freq_hz, f1, f2) in cases {
        let mut reference = reference();
        reference.cal_img(f1, f2);
        let mut sx1261 = get_sx126x();
        sx1261.calibrate_image(freq_hz).await.unwrap();
        assert_eq!(sx1261.take_spi(), reference.inner, "freq {freq_hz}");
    }
}

#[tokio::test]
async fn test_tx_continuous_wave() {
    let mut reference = reference();
    reference.set_tx_cw();
    let mut sx1261 = get_sx126x();
    sx1261.set_tx_continuous_wave_mode().await.unwrap();
    assert_eq!(sx1261.take_spi(), reference.inner);
}

/// Reference-side mirror of one row of datasheet Table 13-21 (PA Operating
/// Modes with Optimal Settings): SetPaConfig values + the SetTxParams power
/// the chip should be handed for a requested dBm. The table itself lives in
/// our driver; SWL2001 leaves it to the BSP layer, so the C driver is fed
/// these values and verifies the command encoding around them.
fn reference_tx_power(
    reference: &mut Context<TestFixture>,
    pa_duty_cycle: u8,
    hp_max: u8,
    device_sel: u8,
    tx_power: i8,
    ramp: sx126x_ramp_time_e,
) {
    reference.set_pa_cfg(&sx126x_pa_cfg_params_t {
        pa_duty_cycle,
        hp_max,
        device_sel,
        pa_lut: 0x01,
    });
    reference.set_tx_params(tx_power, ramp);
}

#[tokio::test]
async fn test_tx_power_sx1261() {
    // (requested dBm, paDutyCycle, hpMax, SetTxParams power)
    // -30 clamps to the low-power PA floor of -17
    let cases = [
        (15i32, 0x06u8, 0x00u8, 14i8),
        (14, 0x04, 0x00, 14),
        (10, 0x01, 0x00, 13),
        (0, 0x01, 0x00, 3),
        (-17, 0x01, 0x00, -14),
        (-30, 0x01, 0x00, -14),
    ];
    for (dbm, duty, hp_max, tx_power) in cases {
        let mut reference = reference();
        reference_tx_power(
            &mut reference,
            duty,
            hp_max,
            1, // device_sel: SX1261
            tx_power,
            sx126x_ramp_time_e::SX126X_RAMP_40_US,
        );
        let mut sx1261 = get_sx126x();
        sx1261.set_tx_power_and_ramp_time(dbm, None, true).await.unwrap();
        assert_eq!(sx1261.take_spi(), reference.inner, "{dbm} dBm");
    }
}

#[tokio::test]
async fn test_tx_power_sx1262() {
    // The high-power path first applies the TX clamp workaround (datasheet
    // §15.2), matching the reference's cfg_tx_clamp read-modify-write.
    // 30 clamps to 22. The discrete SX1262 takes datasheet Table 13-21
    // throughout: the 14 dBm-and-below rows keep PA config 0x02/0x02 and
    // interpolate SetTxParams from the +22 setpoint (txp + 8); the ST table
    // for that row belongs to the Stm32wl variant only.
    let cases = [
        (22i32, 0x04u8, 0x07u8, 22i8),
        (30, 0x04, 0x07, 22),
        (20, 0x03, 0x05, 22),
        (17, 0x02, 0x03, 22),
        (14, 0x02, 0x02, 22),
        (-9, 0x02, 0x02, -1),
    ];
    for (dbm, duty, hp_max, tx_power) in cases {
        let mut reference = reference();
        reference.cfg_tx_clamp();
        reference_tx_power(
            &mut reference,
            duty,
            hp_max,
            0, // device_sel: SX1262
            tx_power,
            sx126x_ramp_time_e::SX126X_RAMP_40_US,
        );
        let mut sx1262 = get_sx1262();
        sx1262.set_tx_power_and_ramp_time(dbm, None, true).await.unwrap();
        assert_eq!(sx1262.take_spi(), reference.inner, "{dbm} dBm");
    }
}

#[tokio::test]
async fn test_tx_power_init_ramp() {
    // Outside TX prep (e.g. at init) the driver uses the slower 200 us ramp
    let mut reference = reference();
    reference_tx_power(&mut reference, 0x01, 0x00, 1, 3, sx126x_ramp_time_e::SX126X_RAMP_200_US);
    let mut sx1261 = get_sx126x();
    sx1261.set_tx_power_and_ramp_time(0, None, false).await.unwrap();
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

#[tokio::test]
async fn test_init_lora_composed() {
    // The whole init_lora sequence against the reference calls chained in
    // the same order. Writes-only comparison, same as the single sync-word
    // test: the C sync word setter is a read-modify-write where ours writes
    // the known reset-derived values directly, so the read halves differ by
    // design (primed with the reset values, the write halves converge).
    const RETENTION_READ: [u8; 4] = [0x1D, 0x02, 0x9F, 0x00];
    const RETENTION_AFTER_RX_GAIN: [u8; 9] = [1, 0x08, 0xAC, 0, 0, 0, 0, 0, 0];

    let mut reference_radio = reference();
    reference_radio.set_dio2_as_rf_sw_ctrl(true);
    reference_radio.set_pkt_type(sx126x_pkt_types_e::SX126X_PKT_TYPE_LORA);
    reference_radio
        .inner
        .prime_read(&[0x1D, 0x07, 0x40, 0x00], &[0x14, 0x24]);
    reference_radio.set_lora_sync_word(0x34);
    reference_radio.set_buffer_base_address(0, 0);
    // Retention list, mirroring ours' one-register-at-a-time factoring; the
    // queued second response lets the second add see the first (the batch
    // form add_registers_to_retention_list(&[both]) would collapse this to
    // one read + one write — a different transaction count by design)
    reference_radio.inner.prime_read(&RETENTION_READ, &[0u8; 9]);
    reference_radio.add_registers_to_retention_list(&[0x08AC]);
    reference_radio
        .inner
        .prime_read(&RETENTION_READ, &RETENTION_AFTER_RX_GAIN);
    reference_radio.add_registers_to_retention_list(&[0x0889]);

    let mut sx1261 = get_sx126x();
    sx1261.spi_mut().prime_read(&RETENTION_READ, &[0u8; 9]);
    sx1261.spi_mut().prime_read(&RETENTION_READ, &RETENTION_AFTER_RX_GAIN);
    sx1261.init_lora(0x34).await.unwrap();

    assert_eq!(sx1261.take_spi().writes(), reference_radio.inner.writes());
}

#[tokio::test]
async fn test_emulated_cad_end_to_end() {
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1261(&chip), true, Delayer).await.unwrap();

    // Quiet channel
    let mdltn_params = modulation(&mut lora);
    lora.prepare_for_cad(&mdltn_params).await.unwrap();
    assert!(!lora.cad(&mdltn_params).await.unwrap());

    // Busy channel
    chip.with_model(|m| m.cad_activity = true);
    lora.prepare_for_cad(&mdltn_params).await.unwrap();
    assert!(lora.cad(&mdltn_params).await.unwrap());
}

#[tokio::test]
async fn test_emulated_rx_single_timeout() {
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1261(&chip), true, Delayer).await.unwrap();

    // Nothing injected: single-mode reception must surface ReceiveTimeout
    // (the model collapses the symbol-timeout wait to zero)
    let mdltn_params = modulation(&mut lora);
    let rx_pkt_params = lora
        .create_rx_packet_params(8, false, 255, true, false, &mdltn_params)
        .unwrap();
    lora.prepare_for_rx(RxMode::Single(8), &mdltn_params, &rx_pkt_params)
        .await
        .unwrap();

    let mut buf = [0u8; 255];
    let result = lora.rx(&rx_pkt_params, &mut buf).await;
    assert!(matches!(result, Err(crate::mod_params::RadioError::ReceiveTimeout)));

    // The same radio still receives fine afterwards
    lora.prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
        .await
        .unwrap();
    chip.inject_rx(b"after-timeout", -90, 2);
    let (len, _) = lora.rx(&rx_pkt_params, &mut buf).await.unwrap();
    assert_eq!(&buf[..len as usize], b"after-timeout");
}

#[tokio::test]
async fn test_tx_power_stm32wl() {
    // The Stm32wl variant takes ST's table: identical to the datasheet on
    // every row except 14 dBm and below, where ST commands the target dBm
    // directly with the same 0x02/0x02 PA config (STM32CubeWL
    // SUBGRF_SetTxParams). Each part uses the table it was characterized
    // with — the STM32WL is an SX126x die integrated into ST's package.
    use crate::sx126x::Stm32wl;
    let cases = [
        (22i32, 0x04u8, 0x07u8, 22i8),
        (17, 0x02, 0x03, 22),
        (14, 0x02, 0x02, 14),
        (-9, 0x02, 0x02, -9),
    ];
    for (dbm, duty, hp_max, tx_power) in cases {
        let mut reference = reference();
        reference.cfg_tx_clamp();
        reference_tx_power(
            &mut reference,
            duty,
            hp_max,
            0, // device_sel: high-power PA
            tx_power,
            sx126x_ramp_time_e::SX126X_RAMP_40_US,
        );
        let mut radio = crate::sx126x::Sx126x::new(
            fixtures::TestFixture::new(),
            fixtures::DummyVariant,
            crate::sx126x::Config {
                chip: Stm32wl {
                    use_high_power_pa: true,
                },
                tcxo_ctrl: None,
                use_dcdc: false,
                rx_boost: true,
            },
        );
        radio.set_tx_power_and_ramp_time(dbm, None, true).await.unwrap();
        assert_eq!(radio.take_spi(), reference.inner, "{dbm} dBm");
    }
}
