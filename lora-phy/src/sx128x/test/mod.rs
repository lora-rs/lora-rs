//! Compare this driver's SPI byte stream against Semtech's reference driver
//! (SWL2001, via the smtc-modem-cores bindings).
//!
//! The sx128x reads are `[opcode, NOP..]` followed by the response bytes in
//! both drivers, so transactions compare one to one. Where our RadioKind
//! method bundles several chip commands, the reference calls are chained in
//! the same order and the whole stream is compared.
mod fixtures;
use fixtures::{CommandStreamFixture, Delayer, Ops, get_sx1280, get_sx1281};

use super::radio_kind_params::PacketType;
use crate::mod_params::{RadioMode, RxMode};
use crate::mod_traits::{IrqState, RadioKind};
use lora_modulation::{Bandwidth, CodingRate, SpreadingFactor};
use smtc_modem_cores::sx128x::{
    Context, sx128x_lora_cad_params_t, sx128x_lora_cad_symbs_e, sx128x_lora_ranging_bw_e, sx128x_lora_ranging_cr_e,
    sx128x_lora_ranging_pkt_len_modes_e, sx128x_lora_ranging_preamble_len_t, sx128x_lora_ranging_sf_e,
    sx128x_mod_params_lora_t, sx128x_pkt_params_lora_t, sx128x_pkt_type_e, sx128x_ramp_time_e, sx128x_reg_mod_e,
    sx128x_standby_cfg_e, sx128x_tick_size_e,
};

const TEST_FREQ_HZ: u32 = 2_450_000_000;

fn reference() -> Context<CommandStreamFixture> {
    Context::new(CommandStreamFixture::new())
}

#[tokio::test]
async fn test_reset() {
    // Reset is the pin toggle and nothing on the bus, as in the reference.
    // LoRa::init() follows it with ensure_ready(radio_mode) for whatever
    // mode the driver was in; a chip put to sleep here would leave BUSY
    // high, and the non-sleep arms of ensure_ready wait on BUSY.
    let mut radio = get_sx1280();
    radio.reset(&mut Delayer).await.unwrap();
    assert!(radio.take_spi().ops.is_empty());
}

#[tokio::test]
async fn test_sleep() {
    // Configuration retained, data buffer flushed (sleepConfig bit 0 only)
    let mut reference = reference();
    reference.set_sleep(false, true);
    let mut radio = get_sx1280();
    radio.set_sleep(false, &mut Delayer).await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_standby() {
    let mut reference = reference();
    reference.set_standby(sx128x_standby_cfg_e::SX128X_STANDBY_CFG_RC);
    let mut radio = get_sx1280();
    radio.set_standby().await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);

    // The variant does not change the command stream
    let mut radio = get_sx1281();
    radio.set_standby().await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_ensure_ready_after_sleep() {
    // The reference wakes the chip through its HAL (an NSS toggle, no bytes),
    // reads the packet type and rewrites the RSSI/SNR fix-up blob when it is
    // LoRa. Ours wakes with a NOP byte on the bus; the stream matches once
    // that NOP is accounted for.
    for packet_type in [PacketType::Gfsk, PacketType::LoRa] {
        let mut fixture = CommandStreamFixture::new();
        fixture.prime_read(&[0x03, 0x00], &[packet_type as u8]);
        let mut reference = Context::new(fixture);
        reference.wakeup();

        let mut radio = get_sx1280();
        radio.spi_mut().prime_read(&[0x03, 0x00], &[packet_type as u8]);
        radio.ensure_ready(RadioMode::Sleep).await.unwrap();
        let ours = radio.take_spi();
        let mut expected = vec![Ops::Write(vec![0x00])];
        expected.extend(reference.inner.ops.iter().cloned());
        assert_eq!(ours.ops, expected, "{packet_type:?}");
    }
}

#[tokio::test]
async fn test_set_channel() {
    // Band edges, a channel centre and two values that do not divide evenly
    // into PLL steps, to pin the rounding
    for freq in [
        2_400_000_000u32,
        TEST_FREQ_HZ,
        2_483_500_000,
        2_402_000_001,
        2_425_123_456,
    ] {
        let mut reference = reference();
        reference.set_rf_freq(freq);
        let mut radio = get_sx1280();
        radio.set_channel(freq).await.unwrap();
        assert_eq!(radio.take_spi(), reference.inner, "freq {freq}");
    }
}

#[tokio::test]
async fn test_modulation_params() {
    use sx128x_lora_ranging_bw_e::*;
    use sx128x_lora_ranging_cr_e::*;
    use sx128x_lora_ranging_sf_e::*;
    let cases = [
        (
            SpreadingFactor::_7,
            Bandwidth::_400KHz,
            CodingRate::_4_5,
            SX128X_LORA_RANGING_SF7,
            SX128X_LORA_RANGING_BW_400,
            SX128X_LORA_RANGING_CR_4_5,
        ),
        (
            SpreadingFactor::_12,
            Bandwidth::_1600KHz,
            CodingRate::_4_8,
            SX128X_LORA_RANGING_SF12,
            SX128X_LORA_RANGING_BW_1600,
            SX128X_LORA_RANGING_CR_4_8,
        ),
        // SF5 and the long-interleaving coding rates exist only on this chip
        (
            SpreadingFactor::_5,
            Bandwidth::_200KHz,
            CodingRate::_4_6,
            SX128X_LORA_RANGING_SF5,
            SX128X_LORA_RANGING_BW_200,
            SX128X_LORA_RANGING_CR_4_6,
        ),
        (
            SpreadingFactor::_9,
            Bandwidth::_800KHz,
            CodingRate::Li4_5,
            SX128X_LORA_RANGING_SF9,
            SX128X_LORA_RANGING_BW_800,
            SX128X_LORA_RANGING_CR_LI_4_5,
        ),
        (
            SpreadingFactor::_10,
            Bandwidth::_800KHz,
            CodingRate::Li4_8,
            SX128X_LORA_RANGING_SF10,
            SX128X_LORA_RANGING_BW_800,
            SX128X_LORA_RANGING_CR_LI_4_8,
        ),
    ];
    for (sf, bw, cr, c_sf, c_bw, c_cr) in cases {
        // SetModulationParams, then the SF-dependent register 0x925 and the
        // read-modify-write of the frequency error correction register 0x93C
        let mut reference = reference();
        reference.set_lora_mod_params(&sx128x_mod_params_lora_t {
            sf: c_sf,
            bw: c_bw,
            cr: c_cr,
        });

        let mut radio = get_sx1280();
        let params = radio.create_modulation_params(sf, bw, cr, TEST_FREQ_HZ).unwrap();
        radio.set_modulation_params(&params).await.unwrap();
        assert_eq!(radio.take_spi(), reference.inner, "sf {sf:?} bw {bw:?} cr {cr:?}");
    }
}

/// `ral_sx128x_convert_lora_pbl_len_from_ral`: the RAL layer is not part of
/// the bindings, so its mantissa/exponent split is restated here. It rounds
/// up to the next representable length and clamps at the chip maximum.
fn reference_preamble(len: u16) -> sx128x_lora_ranging_preamble_len_t {
    if len >= 61440 {
        return sx128x_lora_ranging_preamble_len_t { mant: 15, exp: 12 };
    }
    let mut mant = len;
    let mut exp = 0u8;
    while mant > 1 && mant & 1 == 0 {
        mant >>= 1;
        exp += 1;
    }
    while mant > 15 {
        mant = (mant + 1) >> 1;
        exp += 1;
    }
    sx128x_lora_ranging_preamble_len_t { mant: mant as u8, exp }
}

#[tokio::test]
async fn test_packet_params() {
    // (preamble symbols, implicit header, payload length, crc, inverted iq)
    let cases = [
        (8u16, false, 255u8, true, false),
        (12, false, 32, true, true),
        (16, true, 64, false, false),
        (5, false, 10, true, false),     // odd: rounds up to 6 (mant 3, exp 1)
        (255, false, 10, true, false),   // rounds up to 256
        (65535, false, 10, true, false), // clamps at 61440
    ];
    for (preamble, implicit, len, crc, iq) in cases {
        // SetPacketParams, then the IQ polarity register 0x93B
        let mut reference = reference();
        reference.set_lora_pkt_params(&sx128x_pkt_params_lora_t {
            preamble_len: reference_preamble(preamble),
            header_type: if implicit {
                sx128x_lora_ranging_pkt_len_modes_e::SX128X_LORA_RANGING_PKT_IMPLICIT
            } else {
                sx128x_lora_ranging_pkt_len_modes_e::SX128X_LORA_RANGING_PKT_EXPLICIT
            },
            pld_len_in_bytes: len,
            crc_is_on: crc,
            invert_iq_is_on: iq,
        });

        let mut radio = get_sx1280();
        let mdltn_params = radio
            .create_modulation_params(SpreadingFactor::_7, Bandwidth::_400KHz, CodingRate::_4_5, TEST_FREQ_HZ)
            .unwrap();
        let pkt_params = radio
            .create_packet_params(preamble, implicit, len, crc, iq, &mdltn_params)
            .unwrap();
        radio.set_packet_params(&pkt_params).await.unwrap();
        assert_eq!(
            radio.take_spi(),
            reference.inner,
            "preamble {preamble} implicit {implicit}"
        );
    }
}

#[tokio::test]
async fn test_sync_word() {
    // The reference takes the single-byte form and read-modify-writes the
    // upper nibbles of 0x944/0x945; ours takes the 16-bit register form
    let mut reference = reference();
    reference.set_lora_sync_word(0x34);
    let mut radio = get_sx1280();
    radio.set_lora_sync_word(0x3444).await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);

    // Lower nibbles come from the chip, not the sync word
    let mut fixture = CommandStreamFixture::new();
    fixture.prime_read(&[0x19, 0x09, 0x44, 0x00], &[0x1A, 0x2B]);
    let mut reference = Context::new(fixture);
    reference.set_lora_sync_word(0x12);
    let mut radio = get_sx1280();
    radio.spi_mut().prime_read(&[0x19, 0x09, 0x44, 0x00], &[0x1A, 0x2B]);
    radio.set_lora_sync_word(0x1424).await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
    match reference.inner.writes()[..] {
        [Ops::Write(bytes)] => assert_eq!(bytes, &[0x18, 0x09, 0x44, 0x1A, 0x2B]),
        ref other => panic!("unexpected write stream {other:?}"),
    }
}

#[tokio::test]
async fn test_buffer_base_address() {
    let mut reference = reference();
    reference.set_buffer_base_address(0, 0);
    let mut radio = get_sx1280();
    radio.set_tx_rx_buffer_base_address(0, 0).await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_write_buffer() {
    let payload = b"hello reference driver";
    let mut reference = reference();
    reference.write_buffer(0, payload);
    let mut radio = get_sx1280();
    radio.set_payload(payload).await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_set_tx() {
    // ral_sx128x_set_tx: 1 ms tick base, no timeout
    let mut reference = reference();
    reference.set_tx(sx128x_tick_size_e::SX128X_TICK_SIZE_1000_US, 0);
    let mut radio = get_sx1280();
    radio.do_tx().await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_do_rx() {
    // ral_sx128x_set_rx: 1 ms tick base, 0xFFFF for continuous, else the
    // window in ms. The chip has no symbol-count timeout, so RxMode::Single
    // cannot be expressed natively (see SUPPORTS_TIMED_SINGLE_RX).
    use sx128x_tick_size_e::*;
    let cases = [
        ("continuous", RxMode::Continuous, SX128X_TICK_SIZE_1000_US, 0xFFFF),
        ("single_72ms", RxMode::SingleMs(72), SX128X_TICK_SIZE_1000_US, 72),
        (
            "single_100s",
            RxMode::SingleMs(100_000),
            SX128X_TICK_SIZE_4000_US,
            25_000,
        ),
    ];
    for (label, mode, tick, count) in cases {
        let mut reference = reference();
        reference.set_buffer_base_address(0, 0);
        reference.clear_irq_status(0xFFFF);
        reference.set_rx(tick, count);

        let mut radio = get_sx1280();
        radio.do_rx(mode).await.unwrap();
        assert_eq!(radio.take_spi(), reference.inner, "{label}");
    }
}

#[tokio::test]
async fn test_dio_irq_params() {
    // The masks are this driver's policy; the reference pins the framing.
    // Transmit: TxDone | RxDone | SyncWordValid | SyncWordError | CrcError
    // enabled, TxDone | RxDone routed to DIO1. Receive: RxDone | HeaderValid
    // | HeaderError | CrcError | RxTxTimeout enabled, RxDone | RxTxTimeout
    // routed to DIO1.
    let cases = [
        ("transmit", RadioMode::Transmit, 0x004Fu16, 0x0003u16),
        ("receive", RadioMode::Receive(RxMode::Continuous), 0x4072, 0x4002),
        ("cad", RadioMode::ChannelActivityDetection, 0x3000, 0x3000),
    ];
    for (label, mode, irq_mask, dio1_mask) in cases {
        let mut reference = reference();
        reference.clear_irq_status(0xFFFF);
        reference.set_dio_irq_params(irq_mask, dio1_mask, 0, 0);
        let mut radio = get_sx1280();
        radio.set_irq_params(Some(mode)).await.unwrap();
        assert_eq!(radio.take_spi(), reference.inner, "{label}");
    }
}

#[tokio::test]
async fn test_do_cad() {
    let mut reference = reference();
    reference.set_cad_params(&sx128x_lora_cad_params_t {
        cad_symb_nb: sx128x_lora_cad_symbs_e::SX128X_LORA_CAD_08_SYMB,
    });
    reference.set_cad();
    let mut radio = get_sx1280();
    let mdltn_params = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_400KHz, CodingRate::_4_5, TEST_FREQ_HZ)
        .unwrap();
    radio.do_cad(&mdltn_params).await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);

    // CadDone with CadDetected reports activity
    let mut radio = get_sx1280();
    radio.spi_mut().prime_read(&[0x15, 0x00], &[0x30, 0x00]);
    let mut detected = false;
    let state = radio
        .process_irq_event(RadioMode::ChannelActivityDetection, Some(&mut detected), false)
        .await
        .unwrap();
    assert!(matches!(state, Some(IrqState::Done)));
    assert!(detected);
}

#[tokio::test]
async fn test_clear_irq_status() {
    let mut reference = reference();
    reference.clear_irq_status(0xFFFF);
    let mut radio = get_sx1280();
    radio.clear_irq_status().await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_get_irq_status() {
    let mut radio = get_sx1280();
    radio.spi_mut().prime_read(&[0x15, 0x00], &[0x00, 0x01]); // TxDone
    let state = radio.process_irq_event(RadioMode::Transmit, None, false).await.unwrap();
    assert!(matches!(state, Some(IrqState::Done)));

    let mut fixture = CommandStreamFixture::new();
    fixture.prime_read(&[0x15, 0x00], &[0x00, 0x01]);
    let mut reference = Context::new(fixture);
    let (_, c_irq) = reference.get_irq_status();

    assert_eq!(radio.take_spi(), reference.inner);
    assert_eq!(c_irq, 0x0001);
}

#[tokio::test]
async fn test_get_rx_payload() {
    // Chip reports 4 bytes starting at buffer offset 0x10
    const RX_BUFFER_STATUS: [u8; 2] = [0x17, 0x00];
    let mut radio = get_sx1280();
    radio.spi_mut().prime_read(&RX_BUFFER_STATUS, &[4, 0x10]);
    let mdltn_params = radio
        .create_modulation_params(SpreadingFactor::_7, Bandwidth::_400KHz, CodingRate::_4_5, TEST_FREQ_HZ)
        .unwrap();
    let pkt_params = radio
        .create_packet_params(8, false, 255, true, false, &mdltn_params)
        .unwrap();
    let mut buf = [0u8; 255];
    let len = radio.get_rx_payload(&pkt_params, &mut buf).await.unwrap();
    assert_eq!(len, 4);

    let mut fixture = CommandStreamFixture::new();
    fixture.prime_read(&RX_BUFFER_STATUS, &[4, 0x10]);
    let mut reference = Context::new(fixture);
    let (_, rx_status) = reference.get_rx_buffer_status();
    let mut c_buf = [0u8; 4];
    reference.read_buffer(rx_status.buffer_start_pointer, &mut c_buf);
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_get_packet_status() {
    // Raw chip bytes: rssi 160 (-80 dBm), snr 20 (5 dB), then the three
    // bytes LoRa does not use
    const PKT_STATUS: [u8; 2] = [0x1D, 0x00];
    for (raw_rssi, raw_snr) in [(160u8, 20u8), (161, 0xF0), (1, 0x7F)] {
        let mut radio = get_sx1280();
        radio.spi_mut().prime_read(&PKT_STATUS, &[raw_rssi, raw_snr, 0, 0, 0]);
        let pkt_status = radio.get_rx_packet_status().await.unwrap();

        let mut fixture = CommandStreamFixture::new();
        fixture.prime_read(&PKT_STATUS, &[raw_rssi, raw_snr, 0, 0, 0]);
        let mut reference = Context::new(fixture);
        let (_, c_status) = reference.get_lora_pkt_status();

        assert_eq!(radio.take_spi(), reference.inner);
        assert_eq!(pkt_status.rssi.dbm(), c_status.rssi as i16, "rssi raw {raw_rssi}");
        assert_eq!(pkt_status.snr.db(), c_status.snr as i16, "snr raw {raw_snr}");
    }
}

#[tokio::test]
async fn test_get_rssi() {
    const RSSI_INST: [u8; 2] = [0x1F, 0x00];
    for raw in [161u8, 0, 255] {
        let mut radio = get_sx1280();
        radio.spi_mut().prime_read(&RSSI_INST, &[raw]);
        let rssi = radio.get_rssi().await.unwrap();

        let mut fixture = CommandStreamFixture::new();
        fixture.prime_read(&RSSI_INST, &[raw]);
        let mut reference = Context::new(fixture);
        let (_, c_rssi) = reference.get_rssi_inst();

        assert_eq!(radio.take_spi(), reference.inner);
        assert_eq!(rssi, c_rssi, "raw {raw}");
    }
}

#[tokio::test]
async fn test_tx_power() {
    // The ramp time is board policy (the reference leaves it to the BSP);
    // the power byte is pwr + 18 over the chip's -18..=13 dBm range
    for p_out in [-18i32, 0, 13] {
        let mut reference = reference();
        reference.set_tx_params(p_out as i8, sx128x_ramp_time_e::SX128X_RAMP_10_US);
        let mut radio = get_sx1280();
        radio.set_tx_power_and_ramp_time(p_out, None, true).await.unwrap();
        assert_eq!(radio.take_spi(), reference.inner, "p_out {p_out}");
    }
    // Out of range is an error, not a wrapped byte
    let mut radio = get_sx1280();
    assert!(radio.set_tx_power_and_ramp_time(14, None, true).await.is_err());
    let mut radio = get_sx1280();
    assert!(radio.set_tx_power_and_ramp_time(-19, None, true).await.is_err());
}

#[tokio::test]
async fn test_init_lora_composed() {
    // The whole init_lora sequence against the reference calls chained in
    // the same order: firmware version read, regulator mode (ral_sx128x_init),
    // packet type, sync word, buffer base addresses
    const FW_VERSION_READ: [u8; 4] = [0x19, 0x01, 0xF0, 0x00];
    let fw = b"SX1280 firmware\0";

    let mut reference = reference();
    reference.inner.prime_read(&FW_VERSION_READ, fw);
    let mut c_fw = [0u8; 16];
    reference.read_register(0x01F0, &mut c_fw);
    reference.set_reg_mode(sx128x_reg_mod_e::SX128X_REG_MODE_LDO);
    reference.set_pkt_type(sx128x_pkt_type_e::SX128X_PKT_TYPE_LORA);
    reference.set_lora_sync_word(0x34);
    reference.set_buffer_base_address(0, 0);

    let mut radio = get_sx1280();
    radio.spi_mut().prime_read(&FW_VERSION_READ, fw);
    radio.init_lora(0x3444).await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}

#[tokio::test]
async fn test_tx_continuous_wave() {
    let mut reference = reference();
    reference.set_tx_cw();
    let mut radio = get_sx1280();
    radio.set_tx_continuous_wave_mode().await.unwrap();
    assert_eq!(radio.take_spi(), reference.inner);
}
