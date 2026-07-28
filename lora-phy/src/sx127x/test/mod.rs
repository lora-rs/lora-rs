//! Compare this driver's register traffic against Semtech's reference sx127x
//! driver (SWL2001, via the smtc-modem-cores bindings).
//!
//! The sx127x is register-based and the two drivers factor their traffic
//! differently (burst RMW vs single-register writes, shadow copies vs SPI
//! read-back), so both run against a register-file fixture and equality is
//! on chip-visible outcome: final register file + FIFO stream + IRQ-clear
//! writes. See `fixtures.rs`.
//!
//! The reference is SX1276-only here (our test variant); C composite calls
//! like `sx127x_set_rx` fold in errata our driver applies elsewhere or not
//! at all — noted per test.
mod emulator;
mod fixtures;
use emulator::{get_emulated_sx1276, Chip, Mode};
use fixtures::{get_sx1276, get_sx1276_boost, Delayer, TestFixture};

use crate::mod_params::{RadioMode, RxMode};
use crate::mod_traits::RadioKind;
use crate::sx127x::radio_kind_params::Register;
use crate::LoRa;
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};
use smtc_modem_cores::sx127x::{sx127x_radio_id_e, Context};
use smtc_modem_cores::sys;

/// SX1276 reference with the LoRa packet engine selected. Selecting LoRa is
/// part of both drivers' init flows (ours enters LoRa sleep from reset), and
/// the register file is seeded to that state, so the switch is a no-op on
/// the wire and both sides start identically.
fn reference() -> Context<TestFixture> {
    let mut c = Context::new(TestFixture::new(), sx127x_radio_id_e::SX127X_RADIO_ID_SX1276);
    c.set_pkt_type(sys::sx127x_pkt_types_e_SX127X_PKT_TYPE_LORA);
    c
}

#[tokio::test]
async fn test_set_standby() {
    let mut reference_radio = reference();
    reference_radio.set_standby();

    let mut radio = get_sx1276();
    radio.set_standby().await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi());
    assert_eq!(reference_radio.spi().reg(0x01), 0x81);
}

#[tokio::test]
async fn test_set_sleep() {
    // Standby first so the sleep transition is visible in the register file
    let mut reference_radio = reference();
    reference_radio.set_standby();
    reference_radio.set_sleep();

    let mut radio = get_sx1276();
    radio.set_standby().await.unwrap();
    radio.set_sleep(false, &mut Delayer).await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi());
    assert_eq!(reference_radio.spi().reg(0x01), 0x80);
}

#[tokio::test]
async fn test_set_channel() {
    for freq in [433_000_000u32, 868_100_000, 915_000_000] {
        let mut reference_radio = reference();
        reference_radio.set_rf_freq(freq);

        let mut radio = get_sx1276();
        radio.set_channel(freq).await.unwrap();
        assert_eq!(radio.take_spi(), *reference_radio.spi(), "freq {freq}");
    }
}

async fn compare_mod_params(sf: SpreadingFactor, c_sf: u32, bw: Bandwidth, c_bw: u32, cr: CodingRate, c_cr: u32) {
    let mut reference_radio = reference();
    let ldro = match (sf, bw) {
        // symbol duration > 16 ms
        (SpreadingFactor::_12, Bandwidth::_125KHz) | (SpreadingFactor::_11, Bandwidth::_125KHz) => 1,
        _ => 0,
    };
    reference_radio.set_lora_mod_params(&sys::sx127x_lora_mod_params_t {
        sf: c_sf,
        bw: c_bw,
        cr: c_cr,
        ldro,
    });

    // errata 2.3 registers: SWL2001 writes these on every SetRx, ours with
    // the modulation config; mirror the same end state (none of these
    // bandwidths are 500 kHz, so AutomaticIFOn clears + manual IF 0x40)
    let detect_optimize = reference_radio.spi().reg(0x31);
    reference_radio.write_register(0x31, &[detect_optimize & 0x7F]);
    reference_radio.write_register(0x2F, &[0x40]);
    reference_radio.write_register(0x30, &[0x00]);

    let mut radio = get_sx1276();
    let params = radio.create_modulation_params(sf, bw, cr, 868_100_000).unwrap();
    radio.set_modulation_params(&params).await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi(), "sf {sf:?} bw {bw:?}");
}

#[tokio::test]
async fn test_set_modulation_params() {
    compare_mod_params(
        SpreadingFactor::_7,
        sys::sx127x_lora_sf_e_SX127X_LORA_SF7,
        Bandwidth::_125KHz,
        sys::sx127x_lora_bw_e_SX127X_LORA_BW_125,
        CodingRate::_4_5,
        sys::sx127x_lora_cr_e_SX127X_LORA_CR_4_5,
    )
    .await;
    // LDRO forced on
    compare_mod_params(
        SpreadingFactor::_12,
        sys::sx127x_lora_sf_e_SX127X_LORA_SF12,
        Bandwidth::_125KHz,
        sys::sx127x_lora_bw_e_SX127X_LORA_BW_125,
        CodingRate::_4_8,
        sys::sx127x_lora_cr_e_SX127X_LORA_CR_4_8,
    )
    .await;
    // SF6 selects the SF6 detection optimize + threshold values
    compare_mod_params(
        SpreadingFactor::_6,
        sys::sx127x_lora_sf_e_SX127X_LORA_SF6,
        Bandwidth::_250KHz,
        sys::sx127x_lora_bw_e_SX127X_LORA_BW_250,
        CodingRate::_4_5,
        sys::sx127x_lora_cr_e_SX127X_LORA_CR_4_5,
    )
    .await;
}

#[tokio::test]
async fn test_set_packet_params() {
    // IQ registers are excluded here on the C side: the reference driver
    // only pushes IQ configuration to the chip inside set_tx/set_rx, while
    // ours writes it in set_packet_params. The TX/RX flow tests below cover
    // the IQ register values end to end.
    let mut reference_radio = reference();
    reference_radio.set_lora_pkt_params(&sys::sx127x_lora_pkt_params_t {
        preamble_len_in_symb: 8,
        header_type: sys::sx127x_lora_pkt_len_modes_e_SX127X_LORA_PKT_EXPLICIT,
        pld_len_in_bytes: 32,
        crc_is_on: true,
        invert_iq_is_on: false,
    });
    // ours writes the (unchanged) IQ registers too; mirror the reset values
    reference_radio.write_register(0x33, &[0x27]);
    reference_radio.write_register(0x3B, &[0x1D]);

    // The reference's set_lora_pkt_params is a composite: it forces standby
    // and zeroes both FIFO base addresses (ours does those separately), and
    // pins RegPayloadLength + RegMaxPayloadLength to the packet length even
    // in explicit mode (ours writes RegPayloadLength at set_payload time and
    // leaves MaxPayloadLength at reset, guarding RX size in software).
    let mut radio = get_sx1276();
    let mod_params = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, 868_100_000)
        .unwrap();
    let params = radio
        .create_packet_params(8, false, 32, true, false, &mod_params)
        .unwrap();
    radio.set_standby().await.unwrap();
    radio.set_tx_rx_buffer_base_address(0, 0).await.unwrap();
    radio.set_packet_params(&params).await.unwrap();
    radio.write_register(Register::RegPayloadLength, 32).await.unwrap();
    radio.write_register(Register::RegMaxPayloadLength, 32).await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi());
}

#[tokio::test]
async fn test_set_lora_sync_word() {
    let mut reference_radio = reference();
    reference_radio.set_lora_sync_word(0x34);

    let mut radio = get_sx1276();
    // init_lora also writes the buffer base addresses and reads the version
    radio.init_lora(0x34).await.unwrap();
    reference_radio.write_register(0x0E, &[0x00]);
    reference_radio.write_register(0x0F, &[0x00]);
    assert_eq!(radio.take_spi(), *reference_radio.spi());
}

#[tokio::test]
async fn test_runtime_sync_word() {
    for sync_word in [0x34u8, 0x12] {
        let mut reference_radio = reference();
        reference_radio.set_lora_sync_word(sync_word);

        let mut radio = get_sx1276();
        radio.set_lora_sync_word(sync_word).await.unwrap();
        assert_eq!(radio.take_spi(), *reference_radio.spi(), "sync word {sync_word:#x}");
    }
}

#[tokio::test]
async fn test_symbol_timeout() {
    let mut reference_radio = reference();
    reference_radio.set_lora_sync_timeout(100);

    let mut radio = get_sx1276();
    radio.set_lora_symbol_num_timeout(100).await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi());

    // > 255 exercises the two MSBs in RegModemConfig2
    let mut reference_radio = reference();
    reference_radio.set_lora_sync_timeout(1000);

    let mut radio = get_sx1276();
    radio.set_lora_symbol_num_timeout(1000).await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi());
}

async fn compare_tx_power(boost: bool, p_out: i32, is_20_dbm: bool, ocp_trim: u8) {
    let mut reference_radio = reference();
    reference_radio.set_pa_cfg(&sys::sx127x_pa_cfg_params_t {
        pa_select: if boost {
            sys::sx127x_pa_select_e_SX127X_PA_SELECT_BOOST
        } else {
            sys::sx127x_pa_select_e_SX127X_PA_SELECT_RFO
        },
        is_20_dbm_output_on: is_20_dbm,
    });
    reference_radio.set_tx_params(p_out as i8, sys::sx127x_ramp_time_e_SX127X_RAMP_40_US);
    // OCP policy lives outside the reference driver's TX-params flow; ours
    // sets it alongside TX power. The reference's own set_ocp_value can't be
    // used to mirror it: SWL2001 masks the trim with the inverted-form
    // OCP_TRIM_MASK (~31), zeroing whatever trim is passed — every call
    // writes 0x20 (OcpOn, 45 mA). Raw register write instead.
    reference_radio.write_register(0x0B, &[0x20 | ocp_trim]);
    if boost {
        // MaxPower [6:4] is unused when PA_BOOST is selected: ours writes 0,
        // the reference preserves the reset value via RMW. Align the dead
        // bits so the live ones compare.
        let pa_config = reference_radio.spi().reg(0x09);
        reference_radio.write_register(0x09, &[pa_config & 0b1000_1111]);
    }

    let mut radio = if boost { get_sx1276_boost() } else { get_sx1276() };
    radio.set_tx_power_and_ramp_time(p_out, None, true).await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi(), "boost {boost} p_out {p_out}");
}

#[tokio::test]
async fn test_set_tx_power_boost() {
    // PA_BOOST, high power path (PaDac on, 240 mA OCP)
    compare_tx_power(true, 20, true, 0x1B).await;
    // PA_BOOST standard
    compare_tx_power(true, 14, false, 0x0B).await;
}

#[tokio::test]
async fn test_set_tx_power_rfo() {
    compare_tx_power(false, 14, false, 0x0B).await;
    compare_tx_power(false, 0, false, 0x0B).await;
}

#[tokio::test]
async fn test_tx_flow() {
    // Full TX setup and start: config, payload, IRQ routing, mode switch
    let payload = [0xCA, 0xFE, 0xBA, 0xBE, 0x42];

    let mut reference_radio = reference();
    reference_radio.set_rf_freq(868_100_000);
    reference_radio.set_lora_mod_params(&sys::sx127x_lora_mod_params_t {
        sf: sys::sx127x_lora_sf_e_SX127X_LORA_SF7,
        bw: sys::sx127x_lora_bw_e_SX127X_LORA_BW_125,
        cr: sys::sx127x_lora_cr_e_SX127X_LORA_CR_4_5,
        ldro: 0,
    });
    // errata 2.3 mirror (125 kHz): AutomaticIFOn off + manual IF
    let detect_optimize = reference_radio.spi().reg(0x31);
    reference_radio.write_register(0x31, &[detect_optimize & 0x7F]);
    reference_radio.write_register(0x2F, &[0x40]);
    reference_radio.write_register(0x30, &[0x00]);
    reference_radio.set_lora_pkt_params(&sys::sx127x_lora_pkt_params_t {
        preamble_len_in_symb: 8,
        header_type: sys::sx127x_lora_pkt_len_modes_e_SX127X_LORA_PKT_EXPLICIT,
        pld_len_in_bytes: payload.len() as u8,
        crc_is_on: true,
        invert_iq_is_on: false,
    });
    reference_radio.write_buffer(0, &payload);
    reference_radio.set_irq_mask(sys::sx127x_irq_masks_e_SX127X_IRQ_TX_DONE as u16);
    reference_radio.clear_irq_status(sys::sx127x_irq_masks_e_SX127X_IRQ_ALL as u16);
    reference_radio.set_tx();

    let mut radio = get_sx1276();
    let mod_params = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, 868_100_000)
        .unwrap();
    let pkt_params = radio
        .create_packet_params(8, false, payload.len() as u8, true, false, &mod_params)
        .unwrap();
    radio.set_channel(868_100_000).await.unwrap();
    radio.set_modulation_params(&mod_params).await.unwrap();
    radio.set_packet_params(&pkt_params).await.unwrap();
    radio.set_tx_rx_buffer_base_address(0, 0).await.unwrap();
    radio.set_payload(&payload).await.unwrap();
    radio.set_irq_params(Some(RadioMode::Transmit)).await.unwrap();
    radio.do_tx().await.unwrap();

    // ours leaves RegMaxPayloadLength at reset (RX size is guarded in
    // software); the reference pins it at pkt-params time
    radio
        .write_register(Register::RegMaxPayloadLength, payload.len() as u8)
        .await
        .unwrap();

    let spi = radio.take_spi();
    // ours clears all IRQ flags at the chip (set_irq_params); the reference
    // only does so from its DIO interrupt handlers, outside this flow
    assert_eq!(spi.irq_flags_written, vec![0xFF]);
    assert_eq!(spi, *reference_radio.spi());
    // TX mode, LoRa
    assert_eq!(reference_radio.spi().reg(0x01), 0x83);
    assert_eq!(reference_radio.spi().fifo_written, payload);
}

#[tokio::test]
async fn test_cad_flow() {
    let mut reference_radio = reference();
    reference_radio.set_irq_mask(
        (sys::sx127x_irq_masks_e_SX127X_IRQ_CAD_DONE | sys::sx127x_irq_masks_e_SX127X_IRQ_CAD_DETECTED) as u16,
    );
    reference_radio.clear_irq_status(sys::sx127x_irq_masks_e_SX127X_IRQ_ALL as u16);
    reference_radio.set_cad();

    let mut radio = get_sx1276();
    let mod_params = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, 868_100_000)
        .unwrap();
    radio
        .set_irq_params(Some(RadioMode::ChannelActivityDetection))
        .await
        .unwrap();
    radio.do_cad(&mod_params).await.unwrap();

    // ours re-asserts the (reset-default) LNA gain; the reference leaves it
    reference_radio.write_register(0x0C, &[0x20]);

    assert_eq!(radio.take_spi(), *reference_radio.spi());
    assert_eq!(reference_radio.spi().reg(0x01), 0x87);
}

#[tokio::test]
async fn test_get_rx_payload() {
    let data = [0x01, 0x02, 0x03, 0x04, 0x05];

    // The reference RX read path has no comparable wire footprint: its FIFO
    // drain happens inside the DIO interrupt handlers into a RAM shadow
    // buffer, and both get_rx_buffer_status and read_buffer serve from
    // shadow state. Mirror ours' expected traffic with raw reference
    // accesses instead: read length + current addr, point the FIFO pointer
    // at the packet, drain, park the pointer at 0.
    let mut reference_radio = reference();
    reference_radio.spi_mut().set_reg(0x13, data.len() as u8); // RegRxNbBytes
    reference_radio.spi_mut().set_reg(0x10, 0x40); // RegFifoRxCurrentAddr
    reference_radio.spi_mut().prime_fifo_read(&data);
    let mut reg = [0u8; 1];
    reference_radio.read_register(0x13, &mut reg);
    assert_eq!(reg[0], data.len() as u8);
    reference_radio.read_register(0x10, &mut reg);
    assert_eq!(reg[0], 0x40);
    reference_radio.write_register(0x0D, &[0x40]);
    let mut c_payload = [0u8; 5];
    reference_radio.read_register(0x00, &mut c_payload);
    reference_radio.write_register(0x0D, &[0x00]);

    let mut radio = get_sx1276();
    radio.spi_mut().set_reg(0x13, data.len() as u8);
    radio.spi_mut().set_reg(0x10, 0x40);
    radio.spi_mut().prime_fifo_read(&data);
    let mod_params = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, 868_100_000)
        .unwrap();
    let pkt_params = radio
        .create_packet_params(8, false, 16, true, false, &mod_params)
        .unwrap();
    let mut rx_buffer = [0u8; 16];
    let len = radio.get_rx_payload(&pkt_params, &mut rx_buffer).await.unwrap();

    assert_eq!(radio.take_spi(), *reference_radio.spi());
    assert_eq!(len as usize, data.len());
    assert_eq!(&rx_buffer[..data.len()], &data);
    assert_eq!(&c_payload, &data);
}

#[tokio::test]
async fn test_packet_status_value_parity() {
    // SNR -6 dB (raw = -24 as u8 = 0xE8), RSSI raw 60 @ 868.1 MHz (HF port)
    let mut reference_radio = reference();
    reference_radio.set_rf_freq(868_100_000);
    reference_radio.spi_mut().set_reg(0x19, 0xE8); // RegPktSnrValue
    reference_radio.spi_mut().set_reg(0x1A, 60); // RegPktRssiValue
    let (_, c_status) = reference_radio.get_lora_pkt_status();

    let mut radio = get_sx1276();
    radio.set_channel(868_100_000).await.unwrap();
    radio.spi_mut().set_reg(0x19, 0xE8);
    radio.spi_mut().set_reg(0x1A, 60);
    let status = radio.get_rx_packet_status().await.unwrap();

    assert_eq!(status.snr, c_status.snr_pkt_in_db as i16);
    // both linearize the raw RSSI, with different integer approximations of
    // the same 16/15 correction: ours rounds (raw * 16 + 7) / 15, the
    // reference computes raw + (raw >> 4) — at most 1 dB apart
    assert!(
        (status.rssi - c_status.rssi_pkt_in_dbm as i16).abs() <= 1,
        "rssi {} vs reference {}",
        status.rssi,
        c_status.rssi_pkt_in_dbm
    );
}

#[tokio::test]
async fn test_rssi_value_parity() {
    let mut reference_radio = reference();
    reference_radio.set_rf_freq(868_100_000);
    reference_radio.spi_mut().set_reg(0x1B, 90); // RegRssiValue
    let (_, c_rssi) = reference_radio.get_rssi_inst();

    let mut radio = get_sx1276();
    radio.set_channel(868_100_000).await.unwrap();
    radio.spi_mut().set_reg(0x1B, 90);
    let rssi = radio.get_rssi().await.unwrap();

    assert_eq!(rssi, c_rssi);
}

#[tokio::test]
async fn test_bw500_sensitivity_errata() {
    // Errata 2.1: with 500 kHz bandwidth in the HF band, RegHighBwOptimize1/2
    // take 0x02/0x64. The reference applies this inside its set_rx composite
    // (sx1276_fix_lora_500_khz_bw_sensitivity); ours applies it at
    // set_modulation_params time when the chip version quirk matches, so the
    // reference side mirrors the same register values raw. The frequency
    // gate in ours compared kHz literals against Hz until this test — the
    // errata values were never written on real channels.
    let mut reference_radio = reference();
    reference_radio.set_lora_sync_word(0x34);
    reference_radio.write_register(0x0E, &[0x00]);
    reference_radio.write_register(0x0F, &[0x00]);
    reference_radio.set_lora_mod_params(&sys::sx127x_lora_mod_params_t {
        sf: sys::sx127x_lora_sf_e_SX127X_LORA_SF9,
        bw: sys::sx127x_lora_bw_e_SX127X_LORA_BW_500,
        cr: sys::sx127x_lora_cr_e_SX127X_LORA_CR_4_5,
        ldro: 0,
    });
    reference_radio.write_register(0x36, &[0x02]);
    reference_radio.write_register(0x3A, &[0x64]);

    let mut radio = get_sx1276();
    // init_lora reads the chip version (seeded 0x12) which arms the quirk
    radio.init_lora(0x34).await.unwrap();
    let params = radio
        .create_modulation_params(SpreadingFactor::_9, Bandwidth::_500KHz, CodingRate::_4_5, 868_100_000)
        .unwrap();
    radio.set_modulation_params(&params).await.unwrap();
    assert_eq!(radio.take_spi(), *reference_radio.spi());
    assert_eq!(reference_radio.spi().reg(0x36), 0x02);
    assert_eq!(reference_radio.spi().reg(0x3A), 0x64);
}

#[tokio::test]
async fn test_spurious_rx_errata_round_trip() {
    // Errata 2.3 mirrors SWL2001's sx1276_fix_lora_rx_spurious_signal:
    // 125 kHz clears AutomaticIFOn and programs a manual IF of 0x40/0x00;
    // going back to 500 kHz must re-set AutomaticIFOn. The reference applies
    // this inside set_rx; ours with the modulation config — same end state.
    let mut radio = get_sx1276();
    let params_125 = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, 868_100_000)
        .unwrap();
    let params_500 = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_500KHz, CodingRate::_4_5, 868_100_000)
        .unwrap();

    radio.set_modulation_params(&params_125).await.unwrap();
    assert_eq!(
        radio.spi_mut().reg(0x31) & 0x80,
        0,
        "AutomaticIFOn must clear at 125 kHz"
    );
    assert_eq!(radio.spi_mut().reg(0x2F), 0x40);
    assert_eq!(radio.spi_mut().reg(0x30), 0x00);

    radio.set_modulation_params(&params_500).await.unwrap();
    assert_eq!(
        radio.spi_mut().reg(0x31) & 0x80,
        0x80,
        "AutomaticIFOn must re-arm at 500 kHz"
    );
}

fn emulated_modulation(lora: &mut LoRa<impl RadioKind, Delayer>) -> crate::mod_params::ModulationParams {
    lora.create_modulation_params(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5, 868_100_000)
        .unwrap()
}

#[tokio::test]
async fn test_emulated_tx_end_to_end() {
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1276(&chip), true, Delayer).await.unwrap();

    let mdltn_params = emulated_modulation(&mut lora);
    let mut tx_pkt_params = lora
        .create_tx_packet_params(8, false, true, false, &mdltn_params)
        .unwrap();

    let payload = b"hello sx1276";
    lora.prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 10, payload)
        .await
        .unwrap();
    lora.tx().await.unwrap();

    chip.with_model(|m| {
        assert_eq!(m.tx_log.len(), 1);
        assert_eq!(m.tx_log[0].payload, payload);
        assert_eq!(m.mode(), Mode::Standby);

        // Frequency round-trips through the chip's PLL-step encoding
        let hz = (m.tx_log[0].frequency_raw as u64 * 32_000_000) >> 19;
        assert!((hz as i64 - 868_100_000i64).abs() <= 61, "freq {hz}");
    });
}

#[tokio::test]
async fn test_emulated_rx_end_to_end() {
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1276(&chip), true, Delayer).await.unwrap();

    let mdltn_params = emulated_modulation(&mut lora);
    let rx_pkt_params = lora
        .create_rx_packet_params(8, false, 255, true, false, &mdltn_params)
        .unwrap();
    lora.prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
        .await
        .unwrap();

    // Queued until the chip enters RX, then delivered while the host waits
    // on the DIO0 edge
    chip.inject_rx(b"ping", 100, 5);

    let mut buf = [0u8; 255];
    let (len, status) = lora.rx(&rx_pkt_params, &mut buf).await.unwrap();
    assert_eq!(&buf[..len as usize], b"ping");
    // HF port offset -157, raw 100 linearized by 16/15: -157 + 107
    assert_eq!(status.rssi, -50);
    assert_eq!(status.snr, 5);
}

#[tokio::test]
#[ignore = "documents a known bug: do_rx does not clear stale IRQ flags, so a \
            restarted receive reports the abandoned session's RxDone as a fresh \
            packet. Un-ignore when the stale-flag clear lands."]
async fn test_restarted_rx_ignores_stale_flags() {
    // RegIrqFlags stays latched until the host clears it by writing a 1 —
    // mode changes don't reset it — so a receive that was started but never
    // processed (e.g. its complete_rx future was cancelled by a select
    // timeout) leaves RxDone latched. A restarted receive must not report
    // that stale flag as a fresh packet.
    let chip = Chip::new();
    let mut lora = LoRa::new(get_emulated_sx1276(&chip), true, Delayer).await.unwrap();

    let mdltn_params = emulated_modulation(&mut lora);
    let rx_pkt_params = lora
        .create_rx_packet_params(8, false, 255, true, false, &mdltn_params)
        .unwrap();
    lora.prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
        .await
        .unwrap();
    lora.start_rx().await.unwrap();

    // A packet arrives, latching RxDone — but the caller abandons the
    // receive without ever processing it
    chip.inject_rx(b"stale", 100, 5);

    // ...and later restarts reception. Nothing arrives this session.
    lora.start_rx().await.unwrap();
    let mut buf = [0u8; 255];
    let result = lora.complete_rx(&rx_pkt_params, &mut buf).await.map(|(len, _)| len);
    assert!(
        result.is_err(),
        "reported a phantom packet from a stale RxDone flag: {result:?}"
    );
}

/// Pin every `Register` address to Semtech's SWL2001 register maps
/// (`sx127x_registers_common_e` in sx127x_regs_common_defs.h,
/// `sx127x_registers_lora_e` in sx127x_regs_lora_defs.h). The driver
/// addresses every register through this enum and the emulator's register
/// file is keyed by these bytes, so a wrong address silently reads or writes
/// the wrong register on real hardware. Vendor values are transcribed
/// independently of the driver enum, so a typo in either won't match.
///
/// The driver names the frequency-error registers `RegFreqError*`; the
/// vendor calls the same 0x28..0x2A registers `FEI_*` (frequency error
/// indicator) — same address, different label.
///
/// Not pinned (no entry in the vendor register-address enums, so nothing to
/// compare against): RegModemConfig3 (0x26) and the errata/chip-specific
/// RegIfFreq1/2, RegHighBwOptimize1/2, RegPaDac*, and RegTcxo*. Their bytes
/// are still exercised by the register-file comparison tests above.
#[test]
fn registers_match_swl2001() {
    // (name, our driver's address, SWL2001's address)
    let cases: &[(&str, u8, u8)] = &[
        ("RegFifo", Register::RegFifo as u8, 0x00),
        ("RegOpMode", Register::RegOpMode as u8, 0x01),
        ("RegFrfMsb", Register::RegFrfMsb as u8, 0x06),
        ("RegFrfMid", Register::RegFrfMid as u8, 0x07),
        ("RegFrfLsb", Register::RegFrfLsb as u8, 0x08),
        ("RegPaConfig", Register::RegPaConfig as u8, 0x09),
        ("RegPaRamp", Register::RegPaRamp as u8, 0x0A),
        ("RegOcp", Register::RegOcp as u8, 0x0B),
        ("RegLna", Register::RegLna as u8, 0x0C),
        ("RegFifoAddrPtr", Register::RegFifoAddrPtr as u8, 0x0D),
        ("RegFifoTxBaseAddr", Register::RegFifoTxBaseAddr as u8, 0x0E),
        ("RegFifoRxBaseAddr", Register::RegFifoRxBaseAddr as u8, 0x0F),
        ("RegFifoRxCurrentAddr", Register::RegFifoRxCurrentAddr as u8, 0x10),
        ("RegIrqFlagsMask", Register::RegIrqFlagsMask as u8, 0x11),
        ("RegIrqFlags", Register::RegIrqFlags as u8, 0x12),
        ("RegRxNbBytes", Register::RegRxNbBytes as u8, 0x13),
        ("RegModemStat", Register::RegModemStat as u8, 0x18),
        ("RegPktSnrValue", Register::RegPktSnrValue as u8, 0x19),
        ("RegPktRssiValue", Register::RegPktRssiValue as u8, 0x1A),
        ("RegRssiValue", Register::RegRssiValue as u8, 0x1B),
        ("RegModemConfig1", Register::RegModemConfig1 as u8, 0x1D),
        ("RegModemConfig2", Register::RegModemConfig2 as u8, 0x1E),
        ("RegSymbTimeoutLsb", Register::RegSymbTimeoutLsb as u8, 0x1F),
        ("RegPreambleMsb", Register::RegPreambleMsb as u8, 0x20),
        ("RegPreambleLsb", Register::RegPreambleLsb as u8, 0x21),
        ("RegPayloadLength", Register::RegPayloadLength as u8, 0x22),
        ("RegMaxPayloadLength", Register::RegMaxPayloadLength as u8, 0x23),
        ("RegFreqErrorMsb", Register::RegFreqErrorMsb as u8, 0x28),
        ("RegFreqErrorMid", Register::RegFreqErrorMid as u8, 0x29),
        ("RegFreqErrorLsb", Register::RegFreqErrorLsb as u8, 0x2A),
        ("RegRssiWideband", Register::RegRssiWideband as u8, 0x2C),
        ("RegDetectionOptimize", Register::RegDetectionOptimize as u8, 0x31),
        ("RegInvertiq", Register::RegInvertiq as u8, 0x33),
        ("RegDetectionThreshold", Register::RegDetectionThreshold as u8, 0x37),
        ("RegSyncWord", Register::RegSyncWord as u8, 0x39),
        ("RegInvertiq2", Register::RegInvertiq2 as u8, 0x3B),
        ("RegDioMapping1", Register::RegDioMapping1 as u8, 0x40),
        ("RegVersion", Register::RegVersion as u8, 0x42),
    ];
    for (name, driver, vendor) in cases {
        assert_eq!(
            driver, vendor,
            "Register::{name} = {driver:#04x}, SWL2001 = {vendor:#04x}"
        );
    }
}
