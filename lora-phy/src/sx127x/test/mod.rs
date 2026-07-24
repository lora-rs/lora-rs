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
mod fixtures;
use fixtures::{get_sx1276, get_sx1276_boost, Delayer, TestFixture};

use crate::mod_params::RadioMode;
use crate::mod_traits::RadioKind;
use crate::sx127x::radio_kind_params::Register;
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
