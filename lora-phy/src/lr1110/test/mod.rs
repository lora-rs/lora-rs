//! Compare this driver's SPI byte stream against Semtech's reference lr11xx
//! driver (SWL2001, via the smtc-modem-cores bindings).
//!
//! Both drivers implement the same lr11xx HAL shape — command-write
//! transaction, then a separate read transaction clocking a discarded status
//! byte plus data — so transactions are compared exactly.
mod fixtures;
use fixtures::{
    Delayer, TestFixture, get_lr1110, get_lr1110_boosted, get_lr1110_dcdc_tcxo, get_lr1110_hf, get_lr1110_lp,
};

use crate::mod_params::{RadioMode, RxMode};
use crate::mod_traits::RadioKind;
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};
use smtc_modem_cores::lr11xx::Context;
use smtc_modem_cores::sys;

fn reference() -> Context<TestFixture> {
    Context::new(TestFixture::new())
}

#[tokio::test]
async fn test_set_standby() {
    let mut reference_radio = reference();
    reference_radio.set_standby(sys::lr11xx_system_standby_cfg_t_LR11XX_SYSTEM_STANDBY_CFG_RC);

    let mut radio = get_lr1110();
    radio.set_standby().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_sleep_warm_start() {
    let mut reference_radio = reference();
    reference_radio.set_sleep(
        sys::lr11xx_system_sleep_cfg_t {
            is_warm_start: true,
            is_rtc_timeout: false,
        },
        0,
    );

    let mut radio = get_lr1110();
    radio.set_sleep(true, &mut Delayer).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_sleep_cold_start() {
    let mut reference_radio = reference();
    reference_radio.set_sleep(
        sys::lr11xx_system_sleep_cfg_t {
            is_warm_start: false,
            is_rtc_timeout: false,
        },
        0,
    );

    let mut radio = get_lr1110();
    radio.set_sleep(false, &mut Delayer).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_rf_freq() {
    for freq in [433_000_000u32, 868_100_000, 915_000_000, 2_400_000_000] {
        let mut reference_radio = reference();
        reference_radio.set_rf_freq(freq);

        let mut radio = get_lr1110();
        radio.set_channel(freq).await.unwrap();
        assert_eq!(radio.intf.spi, reference_radio.inner, "freq {freq}");
    }
}

#[tokio::test]
async fn test_set_lora_mod_params() {
    let mut reference_radio = reference();
    reference_radio.set_lora_mod_params(&sys::lr11xx_radio_mod_params_lora_t {
        sf: sys::lr11xx_radio_lora_sf_t_LR11XX_RADIO_LORA_SF10,
        bw: sys::lr11xx_radio_lora_bw_t_LR11XX_RADIO_LORA_BW_125,
        cr: sys::lr11xx_radio_lora_cr_t_LR11XX_RADIO_LORA_CR_4_5,
        ldro: 0,
    });

    let mut radio = get_lr1110();
    let params = radio
        .create_modulation_params(SpreadingFactor::_10, Bandwidth::_125KHz, CodingRate::_4_5, 915_000_000)
        .unwrap();
    radio.set_modulation_params(&params).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_lora_mod_params_ldro() {
    // SF12 @ 125 kHz forces the low-data-rate optimization on
    let mut reference_radio = reference();
    reference_radio.set_lora_mod_params(&sys::lr11xx_radio_mod_params_lora_t {
        sf: sys::lr11xx_radio_lora_sf_t_LR11XX_RADIO_LORA_SF12,
        bw: sys::lr11xx_radio_lora_bw_t_LR11XX_RADIO_LORA_BW_125,
        cr: sys::lr11xx_radio_lora_cr_t_LR11XX_RADIO_LORA_CR_4_8,
        ldro: 1,
    });

    let mut radio = get_lr1110();
    let params = radio
        .create_modulation_params(SpreadingFactor::_12, Bandwidth::_125KHz, CodingRate::_4_8, 868_100_000)
        .unwrap();
    radio.set_modulation_params(&params).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_lora_pkt_params() {
    let mut reference_radio = reference();
    reference_radio.set_lora_pkt_params(&sys::lr11xx_radio_pkt_params_lora_t {
        preamble_len_in_symb: 8,
        header_type: sys::lr11xx_radio_lora_pkt_len_modes_t_LR11XX_RADIO_LORA_PKT_EXPLICIT,
        pld_len_in_bytes: 32,
        crc: sys::lr11xx_radio_lora_crc_t_LR11XX_RADIO_LORA_CRC_ON,
        iq: sys::lr11xx_radio_lora_iq_t_LR11XX_RADIO_LORA_IQ_STANDARD,
    });

    let mut radio = get_lr1110();
    let mod_params = radio
        .create_modulation_params(SpreadingFactor::_10, Bandwidth::_125KHz, CodingRate::_4_5, 915_000_000)
        .unwrap();
    let params = radio
        .create_packet_params(8, false, 32, true, false, &mod_params)
        .unwrap();
    radio.set_packet_params(&params).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_init_lora() {
    // With no TCXO and no DC-DC in the test config, init_lora reduces to the
    // RF-switch DIO setup, packet type, sync word (buffer base addresses do
    // not exist on lr11xx)
    let mut reference_radio = reference();
    reference_radio.set_dio_as_rf_switch(&sys::lr11xx_system_rfswitch_cfg_t {
        enable: 0x01,
        standby: 0x00,
        rx: 0x01,
        tx: 0x02,
        tx_hp: 0x02,
        tx_hf: 0x00,
        gnss: 0x00,
        wifi: 0x00,
    });
    reference_radio.set_pkt_type(sys::lr11xx_radio_pkt_type_t_LR11XX_RADIO_PKT_TYPE_LORA);
    reference_radio.set_lora_sync_word(0x34);

    let mut radio = get_lr1110();
    radio.init_lora(0x3444).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_runtime_sync_word() {
    // The reference API takes the legacy single-byte form, ours the 16-bit
    // sx126x register form; the lr1110 supports the legacy-representable
    // subset (0xY4Z4 -> 0xYZ) and rejects everything else.
    for (sync_word, legacy) in [(0x3444u16, 0x34u8), (0x1424, 0x12)] {
        let mut reference_radio = reference();
        reference_radio.set_lora_sync_word(legacy);

        let mut radio = get_lr1110();
        radio.set_lora_sync_word(sync_word).await.unwrap();
        assert_eq!(radio.intf.spi, reference_radio.inner, "sync word {sync_word:#x}");
    }

    let mut radio = get_lr1110();
    assert_eq!(
        radio.set_lora_sync_word(0xAB12).await,
        Err(crate::mod_params::RadioError::InvalidSyncWord)
    );
}

#[tokio::test]
async fn test_set_tx_power_and_ramp_time_hp() {
    // High-power PA at max output
    let mut reference_radio = reference();
    reference_radio.set_pa_cfg(&sys::lr11xx_radio_pa_cfg_t {
        pa_sel: sys::lr11xx_radio_pa_selection_t_LR11XX_RADIO_PA_SEL_HP,
        pa_reg_supply: sys::lr11xx_radio_pa_reg_supply_t_LR11XX_RADIO_PA_REG_SUPPLY_VBAT,
        pa_duty_cycle: 0x04,
        pa_hp_sel: 0x07,
    });
    reference_radio.set_tx_params(22, sys::lr11xx_radio_ramp_time_t_LR11XX_RADIO_RAMP_208_US);

    let mut radio = get_lr1110();
    radio.set_tx_power_and_ramp_time(22, None, true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);

    // Above-range power clamps to 22 dBm
    let mut radio = get_lr1110();
    radio.set_tx_power_and_ramp_time(30, None, true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_tx_power_and_ramp_time_hp_low_power() {
    // At or below 8 dBm the HP PA runs from VREG for efficiency (vendor BSP
    // LR11XX_PWR_VREG_VBAT_SWITCH = 8). A 5 dBm request maps, per the vendor HP
    // table, to configured power 15, duty 0x04, hp_sel 0x01.
    let mut reference_radio = reference();
    reference_radio.set_pa_cfg(&sys::lr11xx_radio_pa_cfg_t {
        pa_sel: sys::lr11xx_radio_pa_selection_t_LR11XX_RADIO_PA_SEL_HP,
        pa_reg_supply: sys::lr11xx_radio_pa_reg_supply_t_LR11XX_RADIO_PA_REG_SUPPLY_VREG,
        pa_duty_cycle: 0x04,
        pa_hp_sel: 0x01,
    });
    reference_radio.set_tx_params(15, sys::lr11xx_radio_ramp_time_t_LR11XX_RADIO_RAMP_208_US);

    let mut radio = get_lr1110();
    radio.set_tx_power_and_ramp_time(5, None, true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_tx_power_and_ramp_time_lp() {
    // Low-power PA mid range. The vendor LP table (LR11XX_PA_LP_LF_CFG_TABLE)
    // remaps a 10 dBm request to a configured power of 13 with duty 0x01, VREG.
    let mut reference_radio = reference();
    reference_radio.set_pa_cfg(&sys::lr11xx_radio_pa_cfg_t {
        pa_sel: sys::lr11xx_radio_pa_selection_t_LR11XX_RADIO_PA_SEL_LP,
        pa_reg_supply: sys::lr11xx_radio_pa_reg_supply_t_LR11XX_RADIO_PA_REG_SUPPLY_VREG,
        pa_duty_cycle: 0x01,
        pa_hp_sel: 0x00,
    });
    reference_radio.set_tx_params(13, sys::lr11xx_radio_ramp_time_t_LR11XX_RADIO_RAMP_208_US);

    let mut radio = get_lr1110_lp();
    radio.set_tx_power_and_ramp_time(10, None, true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);

    // Max output (+15 dBm) per the LR1110 v2.1 datasheet: configured power 14,
    // duty 0x07, VREG.
    let mut reference_radio = reference();
    reference_radio.set_pa_cfg(&sys::lr11xx_radio_pa_cfg_t {
        pa_sel: sys::lr11xx_radio_pa_selection_t_LR11XX_RADIO_PA_SEL_LP,
        pa_reg_supply: sys::lr11xx_radio_pa_reg_supply_t_LR11XX_RADIO_PA_REG_SUPPLY_VREG,
        pa_duty_cycle: 0x07,
        pa_hp_sel: 0x00,
    });
    reference_radio.set_tx_params(14, sys::lr11xx_radio_ramp_time_t_LR11XX_RADIO_RAMP_208_US);

    let mut radio = get_lr1110_lp();
    radio.set_tx_power_and_ramp_time(15, None, true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);

    // At +15 dBm the LP entry's duty (0x07) exceeds 0x04, which is not allowed
    // below 400 MHz: with a sub-400 MHz modulation the driver rejects it.
    let mut radio = get_lr1110_lp();
    let mod_params = radio
        .create_modulation_params(SpreadingFactor::_10, Bandwidth::_125KHz, CodingRate::_4_5, 169_000_000)
        .unwrap();
    let err = radio
        .set_tx_power_and_ramp_time(15, Some(&mod_params), true)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        crate::mod_params::RadioError::InvalidOutputPowerForFrequency
    ));
}

#[tokio::test]
async fn test_set_tx_power_and_ramp_time_hf() {
    // High-frequency (2.4 GHz) PA at max output. The vendor BSP always supplies
    // the HF PA from VREG (it tops out at +13 dBm, below where VBAT is needed).
    //
    // Values come from the vendor HF table (LR11XX_PA_HF_CFG_TABLE): +13 dBm
    // maps to configured power 13, duty 0x00, hp_sel 0x00.
    let mut reference_radio = reference();
    reference_radio.set_pa_cfg(&sys::lr11xx_radio_pa_cfg_t {
        pa_sel: sys::lr11xx_radio_pa_selection_t_LR11XX_RADIO_PA_SEL_HF,
        pa_reg_supply: sys::lr11xx_radio_pa_reg_supply_t_LR11XX_RADIO_PA_REG_SUPPLY_VREG,
        pa_duty_cycle: 0x00,
        pa_hp_sel: 0x00,
    });
    reference_radio.set_tx_params(13, sys::lr11xx_radio_ramp_time_t_LR11XX_RADIO_RAMP_208_US);

    let mut radio = get_lr1110_hf();
    radio.set_tx_power_and_ramp_time(13, None, true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);

    // Above-range power clamps to 13 dBm
    let mut radio = get_lr1110_hf();
    radio.set_tx_power_and_ramp_time(20, None, true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_do_tx() {
    // do_tx clears all IRQs then starts TX with no timeout
    let mut reference_radio = reference();
    reference_radio.clear_irq_status(0xFFFFFFFF);
    reference_radio.set_tx_with_timeout_in_rtc_step(0);

    let mut radio = get_lr1110();
    radio.do_tx().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_do_rx_continuous() {
    let mut reference_radio = reference();
    reference_radio.stop_timeout_on_preamble(true);
    reference_radio.set_lora_sync_timeout(0);
    reference_radio.set_rx_with_timeout_in_rtc_step(0x00FFFFFF);

    let mut radio = get_lr1110();
    radio.do_rx(RxMode::Continuous).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_do_rx_single() {
    let mut reference_radio = reference();
    reference_radio.stop_timeout_on_preamble(true);
    reference_radio.set_lora_sync_timeout(8);
    reference_radio.set_rx_with_timeout_in_rtc_step(0);

    let mut radio = get_lr1110();
    radio.do_rx(RxMode::Single(8)).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_do_cad() {
    let mut reference_radio = reference();
    reference_radio.set_cad_params(&sys::lr11xx_radio_cad_params_t {
        cad_symb_nb: 0x03, // 8 symbols, log2-coded per UM
        cad_detect_peak: 0x07 + 13,
        cad_detect_min: 10,
        cad_exit_mode: sys::lr11xx_radio_cad_exit_mode_t_LR11XX_RADIO_CAD_EXIT_MODE_STANDBYRC,
        cad_timeout: 0,
    });
    reference_radio.set_cad();

    let mut radio = get_lr1110();
    let params = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, 915_000_000)
        .unwrap();
    radio.do_cad(&params).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_tx_continuous_wave_mode() {
    let mut reference_radio = reference();
    reference_radio.set_tx_cw();

    let mut radio = get_lr1110();
    radio.set_tx_continuous_wave_mode().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_calibrate_image() {
    // Our driver uses the same fixed per-band pairs as SWL2001's
    // ral_lr11xx.c. Note the reference's generic in-MHz helper
    // (lr11xx_system_calibrate_image_in_mhz, floor/ceil of band edges / 4)
    // would land on 0xE8 for 928 MHz — one step short of the 0xE9 both
    // fixed tables use, same story as the sx126x cal_img_in_mhz.
    let mut reference_radio = reference();
    reference_radio.calibrate_image(0xE1, 0xE9);

    let mut radio = get_lr1110();
    radio.calibrate_image(915_000_000).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_irq_params_tx() {
    use crate::lr1110::IrqMask;
    let mask = IrqMask::TxDone.value() | IrqMask::Timeout.value();
    // Our command writes the mask twice: the reference documents the two
    // 32-bit fields as the DIO9 and DIO11 enable masks (not "global +
    // DIO1"), so this equals the reference with the same mask on both
    let mut reference_radio = reference();
    reference_radio.set_dio_irq_params(mask, mask);

    let mut radio = get_lr1110();
    radio.set_irq_params(Some(RadioMode::Transmit)).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_clear_irq_status() {
    let mut reference_radio = reference();
    reference_radio.clear_irq_status(0xFFFFFFFF);

    let mut radio = get_lr1110();
    radio.clear_irq_status().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_get_version() {
    let mut reference_radio = reference();
    reference_radio
        .inner
        .prime_read(&[0x01, 0x01], &[0x22, 0x01, 0x03, 0x08]);
    let (_, c_version) = reference_radio.get_version();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x01, 0x01], &[0x22, 0x01, 0x03, 0x08]);
    let version = radio.get_version().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(version.hw, c_version.hw);
    assert_eq!(version.fw, c_version.fw);
}

#[tokio::test]
async fn test_read_stat1_failure_surfaces() {
    // A Stat1 of CMD_FAIL (bits[3:1] = 0) or CMD_PERR (= 1) means the
    // response data is invalid and must surface as an error.
    for stat1 in [0x00u8, 0x02] {
        let mut radio = get_lr1110();
        radio.intf.spi.stat1 = stat1;
        radio.intf.spi.prime_read(&[0x01, 0x01], &[0x22, 0x01, 0x03, 0x08]);
        let err = radio.get_version().await.unwrap_err();
        assert!(matches!(err, crate::mod_params::RadioError::OpError(s) if s == stat1));
    }
}

#[tokio::test]
async fn test_get_temp_vbat_errors() {
    let mut reference_radio = reference();
    reference_radio.inner.prime_read(&[0x01, 0x1A], &[0x08, 0x50]);
    let (_, c_temp) = reference_radio.get_temp();
    reference_radio.inner.prime_read(&[0x01, 0x19], &[0x83]);
    let (_, c_vbat) = reference_radio.get_vbat();
    reference_radio.inner.prime_read(&[0x01, 0x0D], &[0x00, 0x10]);
    let (_, c_errors) = reference_radio.get_errors();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x01, 0x1A], &[0x08, 0x50]);
    let temp = radio.get_temp().await.unwrap();
    radio.intf.spi.prime_read(&[0x01, 0x19], &[0x83]);
    let vbat = radio.get_vbat().await.unwrap();
    radio.intf.spi.prime_read(&[0x01, 0x0D], &[0x00, 0x10]);
    let errors = radio.get_errors().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(temp, c_temp);
    assert_eq!(vbat, c_vbat);
    assert_eq!(errors, c_errors);
}

#[tokio::test]
async fn test_get_random_number() {
    let mut reference_radio = reference();
    reference_radio
        .inner
        .prime_read(&[0x01, 0x20], &[0xDE, 0xAD, 0xBE, 0xEF]);
    let (_, c_random) = reference_radio.get_random_number();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x01, 0x20], &[0xDE, 0xAD, 0xBE, 0xEF]);
    let random = radio.get_random_number().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    // No value parity here: the reference memcpys the response into a u32,
    // so its value is host-endian (0xEFBEADDE on little-endian) while we
    // parse big-endian like every other multi-byte field. Both are equally
    // random; only the wire bytes are comparable.
    assert_eq!(random, u32::from_be_bytes([0xDE, 0xAD, 0xBE, 0xEF]));
    assert_eq!(c_random, u32::from_ne_bytes([0xDE, 0xAD, 0xBE, 0xEF]));
}

#[tokio::test]
async fn test_read_uid_and_join_eui() {
    let uid = [1, 2, 3, 4, 5, 6, 7, 8];
    let mut reference_radio = reference();
    reference_radio.inner.prime_read(&[0x01, 0x25], &uid);
    let (_, c_uid) = reference_radio.read_uid();
    reference_radio.inner.prime_read(&[0x01, 0x26], &uid);
    let (_, c_eui) = reference_radio.read_join_eui();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x01, 0x25], &uid);
    let our_uid = radio.read_uid().await.unwrap();
    radio.intf.spi.prime_read(&[0x01, 0x26], &uid);
    let our_eui = radio.read_join_eui().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(our_uid, c_uid);
    assert_eq!(our_eui, c_eui);
}

#[tokio::test]
async fn test_get_status_direct_read() {
    // Both drivers read stat1 + stat2 + 4 irq bytes in one command-less
    // direct read
    let wire = [0x02, 0x44, 0x00, 0x00, 0x00, 0x14];
    let mut reference_radio = reference();
    reference_radio.inner.prime_direct_read(&wire);
    let (_, _stat1, _stat2, c_irq) = reference_radio.get_status();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_direct_read(&wire);
    let irq_flags = radio.get_irq_flags().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(irq_flags, c_irq);
}

#[tokio::test]
async fn test_get_rx_packet_status() {
    let mut reference_radio = reference();
    reference_radio.inner.prime_read(&[0x02, 0x04], &[80, 20, 82]);
    let (_, c_status) = reference_radio.get_lora_pkt_status();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x02, 0x04], &[80, 20, 82]);
    let status = radio.get_rx_packet_status().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(status.rssi, c_status.rssi_pkt_in_dbm as i16);
    assert_eq!(status.snr, c_status.snr_pkt_in_db as i16);
}

#[tokio::test]
async fn test_get_rssi() {
    let mut reference_radio = reference();
    reference_radio.inner.prime_read(&[0x02, 0x05], &[100]);
    let (_, c_rssi) = reference_radio.get_rssi_inst();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x02, 0x05], &[100]);
    let rssi = radio.get_rssi().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(rssi, c_rssi as i16);
}

#[tokio::test]
async fn test_set_payload() {
    let payload = [0xCA, 0xFE, 0xBA, 0xBE, 0x42];
    let mut reference_radio = reference();
    reference_radio.regmem_write_buffer8(&payload);

    let mut radio = get_lr1110();
    radio.set_payload(&payload).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_get_rx_payload() {
    let data = [0xCA, 0xFE, 0xBA, 0xBE, 0x42];
    let mut reference_radio = reference();
    reference_radio.inner.prime_read(&[0x02, 0x03], &[5, 0]);
    let (_, c_buf_status) = reference_radio.get_rx_buffer_status();
    reference_radio.inner.prime_read(&[0x01, 0x0A, 0, 5], &data);
    let mut c_payload = [0u8; 5];
    reference_radio.regmem_read_buffer8(c_buf_status.buffer_start_pointer, &mut c_payload);

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x02, 0x03], &[5, 0]);
    radio.intf.spi.prime_read(&[0x01, 0x0A, 0, 5], &data);
    let mut rx_buffer = [0u8; 16];
    let pkt_params = {
        let mod_params = radio
            .create_modulation_params(SpreadingFactor::_10, Bandwidth::_125KHz, CodingRate::_4_5, 915_000_000)
            .unwrap();
        radio
            .create_packet_params(8, false, 16, true, false, &mod_params)
            .unwrap()
    };
    let len = radio.get_rx_payload(&pkt_params, &mut rx_buffer).await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(len, 5);
    assert_eq!(&rx_buffer[..5], &c_payload);
    assert_eq!(&rx_buffer[..5], &data);
}

#[tokio::test]
async fn test_high_acp_workaround() {
    let mut reference_radio = reference();
    reference_radio.apply_high_acp_workaround();

    let mut radio = get_lr1110();
    radio.apply_high_acp_workaround().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_init_system_dcdc_tcxo() {
    // BRD_TCXO_WAKEUP_TIME (10 ms) in RTC steps
    let tcxo_timeout = 10 * 32768 / 1000;
    let mut reference_radio = reference();
    reference_radio.set_reg_mode(sys::lr11xx_system_reg_mode_t_LR11XX_SYSTEM_REG_MODE_DCDC);
    reference_radio.clear_errors();
    reference_radio.set_tcxo_mode(
        sys::lr11xx_system_tcxo_supply_voltage_t_LR11XX_SYSTEM_TCXO_CTRL_1_8V,
        tcxo_timeout,
    );
    reference_radio.calibrate(0b0111_1111);

    let mut radio = get_lr1110_dcdc_tcxo();
    radio.init_system().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_do_tx_tcxo() {
    // With a TCXO the driver re-arms it with a longer timeout before TX
    let mut reference_radio = reference();
    reference_radio.clear_irq_status(0xFFFFFFFF);
    reference_radio.set_tcxo_mode(
        sys::lr11xx_system_tcxo_supply_voltage_t_LR11XX_SYSTEM_TCXO_CTRL_1_8V,
        0x000CD0,
    );
    reference_radio.set_tx_with_timeout_in_rtc_step(0);

    let mut radio = get_lr1110_dcdc_tcxo();
    radio.do_tx().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_do_rx_boosted() {
    let mut reference_radio = reference();
    reference_radio.stop_timeout_on_preamble(true);
    reference_radio.set_lora_sync_timeout(0);
    reference_radio.cfg_rx_boosted(true);
    reference_radio.set_rx_with_timeout_in_rtc_step(0x00FFFFFF);

    let mut radio = get_lr1110_boosted();
    radio.do_rx(RxMode::Continuous).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_do_rx_duty_cycle() {
    use crate::mod_params::DutyCycleParams;
    let mut reference_radio = reference();
    reference_radio.stop_timeout_on_preamble(true);
    reference_radio.set_lora_sync_timeout(0);
    // ms-based reference entry point; converts with the same *32768/1000.
    // Neither driver applies the high-ACP workaround on the duty-cycle path.
    reference_radio.set_rx_duty_cycle(
        100,
        900,
        sys::lr11xx_radio_rx_duty_cycle_mode_t_LR11XX_RADIO_RX_DUTY_CYCLE_MODE_RX,
    );

    let mut radio = get_lr1110();
    radio
        .do_rx(RxMode::DutyCycle(DutyCycleParams {
            rx_time: 100,
            sleep_time: 900,
        }))
        .await
        .unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_auto_tx_rx() {
    use crate::lr1110::radio_kind_params::IntermediaryMode;
    let mut reference_radio = reference();
    reference_radio.auto_tx_rx(
        1000,
        sys::lr11xx_radio_intermediary_mode_t_LR11XX_RADIO_MODE_STANDBY_RC,
        2000,
    );

    let mut radio = get_lr1110();
    radio
        .set_auto_tx_rx(1000, IntermediaryMode::StandbyRc, 2000)
        .await
        .unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_rx_tx_fallback_mode() {
    use crate::lr1110::radio_kind_params::FallbackMode;
    let mut reference_radio = reference();
    reference_radio.set_rx_tx_fallback_mode(sys::lr11xx_radio_fallback_modes_t_LR11XX_RADIO_FALLBACK_STDBY_XOSC);

    let mut radio = get_lr1110();
    radio.set_rx_tx_fallback_mode(FallbackMode::StandbyXosc).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_standby_xosc() {
    let mut reference_radio = reference();
    reference_radio.set_standby(sys::lr11xx_system_standby_cfg_t_LR11XX_SYSTEM_STANDBY_CFG_XOSC);

    let mut radio = get_lr1110();
    radio.set_standby_mode(true).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_gfsk_mod_params() {
    use crate::lr1110::{GfskBandwidth, GfskModulationParams, GfskPulseShape};
    // The LR11xx takes bitrate in raw bps and deviation in raw Hz (no
    // SX126x-style chip-format conversion)
    let mut reference_radio = reference();
    reference_radio.set_gfsk_mod_params(&sys::lr11xx_radio_mod_params_gfsk_t {
        br_in_bps: 50_000,
        pulse_shape: sys::lr11xx_radio_gfsk_pulse_shape_t_LR11XX_RADIO_GFSK_PULSE_SHAPE_BT_1,
        bw_dsb_param: sys::lr11xx_radio_gfsk_bw_t_LR11XX_RADIO_GFSK_BW_117300,
        fdev_in_hz: 25_000,
    });

    let mut radio = get_lr1110();
    radio
        .set_gfsk_mod_params(&GfskModulationParams {
            bitrate_bps: 50_000,
            pulse_shape: GfskPulseShape::Bt1,
            bandwidth: GfskBandwidth::Bw117300,
            freq_dev_hz: 25_000,
        })
        .await
        .unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_gfsk_pkt_params() {
    use crate::lr1110::{
        GfskAddressFiltering, GfskCrcType, GfskDcFree, GfskHeaderType, GfskPacketParams, GfskPreambleDetector,
    };
    let mut reference_radio = reference();
    reference_radio.set_gfsk_pkt_params(&sys::lr11xx_radio_pkt_params_gfsk_t {
        preamble_len_in_bits: 40,
        preamble_detector: sys::lr11xx_radio_gfsk_preamble_detector_t_LR11XX_RADIO_GFSK_PREAMBLE_DETECTOR_MIN_16BITS,
        sync_word_len_in_bits: 24,
        address_filtering: sys::lr11xx_radio_gfsk_address_filtering_t_LR11XX_RADIO_GFSK_ADDRESS_FILTERING_DISABLE,
        header_type: sys::lr11xx_radio_gfsk_pkt_len_modes_t_LR11XX_RADIO_GFSK_PKT_VAR_LEN,
        pld_len_in_bytes: 64,
        crc_type: sys::lr11xx_radio_gfsk_crc_type_t_LR11XX_RADIO_GFSK_CRC_2_BYTES,
        dc_free: sys::lr11xx_radio_gfsk_dc_free_t_LR11XX_RADIO_GFSK_DC_FREE_WHITENING,
    });

    let mut radio = get_lr1110();
    radio
        .set_gfsk_pkt_params(&GfskPacketParams {
            preamble_length: 40,
            preamble_detector: GfskPreambleDetector::Bits16,
            sync_word_length_bits: 24,
            address_filtering: GfskAddressFiltering::Disabled,
            header_type: GfskHeaderType::Variable,
            payload_length: 64,
            crc_type: GfskCrcType::Crc2Bytes,
            dc_free: GfskDcFree::Whitening,
        })
        .await
        .unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_gfsk_sync_word() {
    let sync_word = [0x97, 0x23, 0x52, 0x25, 0x56, 0x53, 0x65, 0x64];
    let mut reference_radio = reference();
    reference_radio.set_gfsk_sync_word(&sync_word);

    let mut radio = get_lr1110();
    radio.set_gfsk_sync_word(&sync_word).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);

    // A shorter sync word still puts 8 (zero-padded) bytes on the wire
    let mut reference_radio = reference();
    reference_radio.set_gfsk_sync_word(&[0xC1, 0x94, 0xC1, 0, 0, 0, 0, 0]);

    let mut radio = get_lr1110();
    radio.set_gfsk_sync_word(&[0xC1, 0x94, 0xC1]).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_set_gfsk_crc_whitening_address() {
    let mut reference_radio = reference();
    reference_radio.set_gfsk_crc_params(0x1D0F_1D0F, 0x1021_1021);
    reference_radio.set_gfsk_whitening_seed(0x01FF);
    reference_radio.set_pkt_address(0x42, 0xFF);

    let mut radio = get_lr1110();
    radio.set_gfsk_crc_params(0x1D0F_1D0F, 0x1021_1021).await.unwrap();
    radio.set_gfsk_whitening_params(0x01FF).await.unwrap();
    radio.set_gfsk_pkt_address(0x42, 0xFF).await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_get_gfsk_packet_status() {
    // Response layout: rssi_sync, rssi_avg, rx_len, status bits
    let mut reference_radio = reference();
    reference_radio.inner.prime_read(&[0x02, 0x04], &[80, 84, 17, 0x02]);
    let (_, c_status) = reference_radio.get_gfsk_pkt_status();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x02, 0x04], &[80, 84, 17, 0x02]);
    let (rx_len, rssi_sync, rssi_avg) = radio.get_gfsk_packet_status().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(rx_len, c_status.rx_len_in_bytes);
    assert_eq!(rssi_sync, c_status.rssi_sync_in_dbm as i16);
    assert_eq!(rssi_avg, c_status.rssi_avg_in_dbm as i16);
}

#[tokio::test]
async fn test_stats() {
    let lora_stats = [0x00, 0x0A, 0x00, 0x02, 0x00, 0x01, 0x00, 0x03];
    let mut reference_radio = reference();
    reference_radio.inner.prime_read(&[0x02, 0x01], &lora_stats);
    let (_, c_stats) = reference_radio.get_lora_stats();
    reference_radio.reset_stats();

    let mut radio = get_lr1110();
    radio.intf.spi.prime_read(&[0x02, 0x01], &lora_stats);
    let stats = radio.get_lora_stats().await.unwrap();
    radio.reset_stats().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(stats.nb_pkt_received, c_stats.nb_pkt_received);
    assert_eq!(stats.nb_pkt_crc_error, c_stats.nb_pkt_crc_error);
    assert_eq!(stats.nb_pkt_header_error, c_stats.nb_pkt_header_error);
    assert_eq!(stats.nb_pkt_false_sync, c_stats.nb_pkt_falsesync);
}

#[tokio::test]
async fn test_lr_fhss_init() {
    let mut reference_radio = reference();
    reference_radio.lr_fhss_init();

    let mut radio = get_lr1110();
    radio.lr_fhss_init().await.unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_lr_fhss_build_frame() {
    use crate::lr1110::{
        LR_FHSS_DEFAULT_SYNC_WORD, LrFhssBandwidth, LrFhssCodingRate, LrFhssGrid, LrFhssModulationType, LrFhssParams,
        LrFhssV1Params,
    };
    let payload = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

    let mut reference_radio = reference();
    reference_radio.lr_fhss_build_frame(
        &sys::lr11xx_lr_fhss_params_t {
            lr_fhss_params: sys::lr_fhss_v1_params_t {
                sync_word: LR_FHSS_DEFAULT_SYNC_WORD.as_ptr(),
                modulation_type: sys::lr_fhss_v1_modulation_type_e_LR_FHSS_V1_MODULATION_TYPE_GMSK_488,
                cr: sys::lr_fhss_v1_cr_e_LR_FHSS_V1_CR_2_3,
                grid: sys::lr_fhss_v1_grid_e_LR_FHSS_V1_GRID_25391_HZ,
                bw: sys::lr_fhss_v1_bw_e_LR_FHSS_V1_BW_136719_HZ,
                enable_hopping: true,
                header_count: 3,
            },
            device_offset: 0,
        },
        42,
        &payload,
    );

    let mut radio = get_lr1110();
    radio
        .lr_fhss_build_frame(
            &LrFhssParams {
                lr_fhss_params: LrFhssV1Params {
                    sync_word: LR_FHSS_DEFAULT_SYNC_WORD,
                    modulation_type: LrFhssModulationType::Gmsk488,
                    coding_rate: LrFhssCodingRate::Cr2_3,
                    grid: LrFhssGrid::Grid25391Hz,
                    enable_hopping: true,
                    bandwidth: LrFhssBandwidth::Bw136719Hz,
                    header_count: 3,
                },
                device_offset: 0,
            },
            42,
            &payload,
        )
        .await
        .unwrap();
    assert_eq!(radio.intf.spi, reference_radio.inner);
}

#[tokio::test]
async fn test_regmem() {
    let words = [0xDEAD_BEEF_u32, 0xCAFE_BABE];
    let mem = [0x11, 0x22, 0x33, 0x44];

    let mut reference_radio = reference();
    reference_radio.regmem_write_regmem32(0x00F3_0054, &words);
    reference_radio.inner.prime_read(
        &[0x01, 0x06, 0x00, 0xF3, 0x00, 0x54, 2],
        &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE],
    );
    let mut c_words = [0u32; 2];
    reference_radio.regmem_read_regmem32(0x00F3_0054, &mut c_words);
    reference_radio.regmem_write_mem8(0x0080_0000, &mem);
    reference_radio
        .inner
        .prime_read(&[0x01, 0x08, 0x00, 0x80, 0x00, 0x00, 4], &mem);
    let mut c_mem = [0u8; 4];
    reference_radio.regmem_read_mem8(0x0080_0000, &mut c_mem);
    reference_radio.regmem_clear_rxbuffer();

    let mut radio = get_lr1110();
    radio.regmem_write_regmem32(0x00F3_0054, &words).await.unwrap();
    radio.intf.spi.prime_read(
        &[0x01, 0x06, 0x00, 0xF3, 0x00, 0x54, 2],
        &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE],
    );
    let mut our_words = [0u32; 2];
    radio.regmem_read_regmem32(0x00F3_0054, &mut our_words).await.unwrap();
    radio.regmem_write_mem8(0x0080_0000, &mem).await.unwrap();
    radio
        .intf
        .spi
        .prime_read(&[0x01, 0x08, 0x00, 0x80, 0x00, 0x00, 4], &mem);
    let mut our_mem = [0u8; 4];
    radio.regmem_read_mem8(0x0080_0000, &mut our_mem).await.unwrap();
    radio.regmem_clear_rxbuffer().await.unwrap();

    assert_eq!(radio.intf.spi, reference_radio.inner);
    assert_eq!(our_words, c_words);
    assert_eq!(our_words, words);
    assert_eq!(our_mem, c_mem);
    assert_eq!(our_mem, mem);
}
