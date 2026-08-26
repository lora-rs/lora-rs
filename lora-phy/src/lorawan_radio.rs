#![allow(missing_docs)]

use super::mod_params::{PacketParams, RadioError};
use super::mod_traits::RadioKind;
use super::{DelayNs, LoRa, RxMode};

use lorawan_device::async_device::{
    RxWindowTiming, Timings,
    radio::{PhyRxTx, RxConfig, RxMode as LorawanRxMode, RxQuality, RxStatus, TxConfig},
};

const DEFAULT_RX_WINDOW_LEAD_TIME: u32 = 50;
const DEFAULT_SYSTEM_MAX_RX_ERROR_MS: u32 = 10;

/// LoRaWAN radio implementation.
///
/// The const generic P is the max power the radio may be instructed to transmit at.
/// The const generic G is the antenna gain and board loss in dBi.
pub struct LorawanRadio<RK, DLY, const P: u8, const G: i8 = 0>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    pub(crate) lora: LoRa<RK, DLY>,
    rx_pkt_params: Option<PacketParams>,
    rx_window_lead_time: u32,
    rx_window_buffer: u32,
    min_rx_symbols: u16,
    system_max_rx_error_ms: u32,
}

impl<RK, DLY, const P: u8, const G: i8> From<LoRa<RK, DLY>> for LorawanRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    fn from(lora: LoRa<RK, DLY>) -> Self {
        Self {
            lora,
            rx_pkt_params: None,
            rx_window_lead_time: DEFAULT_RX_WINDOW_LEAD_TIME,
            rx_window_buffer: DEFAULT_RX_WINDOW_LEAD_TIME,
            min_rx_symbols: RK::DEFAULT_MIN_RX_SYMBOLS,
            system_max_rx_error_ms: DEFAULT_SYSTEM_MAX_RX_ERROR_MS,
        }
    }
}

impl<RK, DLY, const P: u8, const G: i8> LorawanRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    pub fn set_rx_window_lead_time(&mut self, lt: u32) {
        self.rx_window_lead_time = lt;
    }

    /// Set the maximum expected clock and scheduling error on either side of
    /// the nominal receive-window time. The default is 10 ms, matching
    /// LoRaMac-node.
    pub fn set_system_max_rx_error(&mut self, error_ms: u32) {
        self.system_max_rx_error_ms = error_ms;
    }

    /// Set the minimum preamble-detection timeout.
    pub fn set_min_rx_symbols(&mut self, symbols: u16) {
        self.min_rx_symbols = symbols.max(5);
    }

    pub fn set_rx_window_buffer(&mut self, buffer: u32) {
        self.rx_window_buffer = buffer;
    }
}

/// Provide the timing values
impl<RK, DLY, const P: u8, const G: i8> Timings for LorawanRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    fn get_rx_window_buffer(&self) -> u32 {
        self.rx_window_buffer
    }

    fn get_rx_window_lead_time_ms(&self) -> u32 {
        self.rx_window_lead_time
    }

    fn get_rx_window_timing(&self, rf: &lorawan_device::async_device::radio::RfConfig) -> RxWindowTiming {
        compute_rx_window_timing(
            rf.bb.symbol_duration_us(),
            self.min_rx_symbols,
            self.system_max_rx_error_ms,
            self.rx_window_lead_time,
        )
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error {
    Radio(RadioError),
    NoRxParams,
}

impl From<RadioError> for Error {
    fn from(err: RadioError) -> Self {
        Error::Radio(err)
    }
}

/// Provide the LoRa physical layer rx/tx interface
impl<RK, DLY, const P: u8, const G: i8> PhyRxTx for LorawanRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    type PhyError = Error;

    const ANTENNA_GAIN: i8 = G;

    const MAX_RADIO_POWER: u8 = P;

    async fn tx(&mut self, config: TxConfig, buffer: &[u8]) -> Result<u32, Self::PhyError> {
        let mdltn_params = self.lora.create_modulation_params(
            config.rf.bb.sf,
            config.rf.bb.bw,
            config.rf.bb.cr,
            config.rf.frequency,
        )?;
        let mut tx_pkt_params = self
            .lora
            .create_tx_packet_params(8, false, true, false, &mdltn_params)?;

        self.lora
            .prepare_for_tx(&mdltn_params, &mut tx_pkt_params, config.pw.into(), buffer)
            .await?;
        self.lora.tx().await?;
        Ok(0)
    }

    async fn setup_rx(&mut self, config: RxConfig) -> Result<(), Self::PhyError> {
        let mode = RxMode::from(config.mode, config.rf.bb);
        self.setup_rx_mode(config, mode).await
    }

    async fn setup_rx_window(&mut self, config: RxConfig, timing: RxWindowTiming) -> Result<(), Self::PhyError> {
        let mode = select_single_rx_mode(
            timing.timeout_symbols,
            config.rf.bb.symbol_duration_us(),
            RK::MAX_SINGLE_RX_SYMBOLS,
            RK::SUPPORTS_TIMED_SINGLE_RX,
        );
        self.setup_rx_mode(config, mode).await
    }

    async fn rx_single(&mut self, buf: &mut [u8]) -> Result<RxStatus, Self::PhyError> {
        if let Some(rx_params) = &self.rx_pkt_params {
            match self.lora.rx(rx_params, buf).await {
                Ok((len, q)) => Ok(RxStatus::Rx(len as usize, RxQuality::new(q.rssi, q.snr as i8))),
                Err(RadioError::ReceiveTimeout) => Ok(RxStatus::RxTimeout),
                Err(err) => Err(err.into()),
            }
        } else {
            Err(Error::NoRxParams)
        }
    }
    async fn rx_continuous(&mut self, receiving_buffer: &mut [u8]) -> Result<(usize, RxQuality), Self::PhyError> {
        if let Some(rx_params) = &self.rx_pkt_params {
            match self.lora.rx(rx_params, receiving_buffer).await {
                Ok((received_len, rx_pkt_status)) => {
                    Ok((
                        received_len as usize,
                        RxQuality::new(rx_pkt_status.rssi, rx_pkt_status.snr as i8), // downcast snr
                    ))
                }
                Err(err) => Err(err.into()),
            }
        } else {
            Err(Error::NoRxParams)
        }
    }
    async fn low_power(&mut self) -> Result<(), Self::PhyError> {
        self.lora.sleep(false).await.map_err(|e| e.into())
    }
    async fn low_power_retain(&mut self) -> Result<(), Self::PhyError> {
        // Warm sleep between the transmit and its receive windows on chips that
        // retain their configuration, so the next setup is a warm start
        // (hundreds of us) instead of a cold re-init (tens of ms). Chips without
        // a retention sleep must re-initialize after every sleep, so they take
        // the normal cold sleep here.
        self.lora.sleep(RK::SUPPORTS_WARM_START).await.map_err(|e| e.into())
    }
}

impl<RK, DLY, const P: u8, const G: i8> LorawanRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    async fn setup_rx_mode(&mut self, config: RxConfig, mode: RxMode) -> Result<(), Error> {
        let mdltn_params = self.lora.create_modulation_params(
            config.rf.bb.sf,
            config.rf.bb.bw,
            config.rf.bb.cr,
            config.rf.frequency,
        )?;
        let rx_pkt_params = self
            .lora
            .create_rx_packet_params(8, false, 255, true, true, &mdltn_params)?;
        self.lora.prepare_for_rx(mode, &mdltn_params, &rx_pkt_params).await?;
        self.rx_pkt_params = Some(rx_pkt_params);
        Ok(())
    }
}

impl RxMode {
    fn from(mode: LorawanRxMode, bb: lora_modulation::BaseBandModulationParams) -> Self {
        match mode {
            LorawanRxMode::Continuous => RxMode::Continuous,
            LorawanRxMode::Single { ms } => {
                const PREAMBLE_SYMBOLS: u16 = 13;
                RxMode::Single(PREAMBLE_SYMBOLS.saturating_add(bb.delay_in_symbols_ceil(ms)))
            }
        }
    }
}

/// Choose how the chip should close a receive window `timeout_symbols` long.
///
/// At fast data rates the window span can exceed what the chip's symbol
/// counter can express (SF7/BW500 with the default lead and error wants 280+
/// symbols against the sx126x/lr11xx ceiling of 248). Letting the driver clamp
/// there would close the window before a latest-edge packet inside the
/// configured error bound arrives, so fall back to the wall-clock RX timer,
/// which covers the same span and stops on preamble detection just like the
/// symbol timeout. Chips without that timer keep the clamped symbol timeout;
/// their windows shorten by the clamped amount at the late edge.
fn select_single_rx_mode(
    timeout_symbols: u16,
    symbol_duration_us: u32,
    max_symbols: u16,
    timed_single_rx: bool,
) -> RxMode {
    if timeout_symbols <= max_symbols {
        RxMode::Single(timeout_symbols)
    } else if timed_single_rx {
        let ms = (timeout_symbols as u64 * symbol_duration_us as u64).div_ceil(1_000) as u32;
        RxMode::SingleMs(ms)
    } else {
        warn!(
            "rx window of {} symbols exceeds the chip's {}-symbol timeout ceiling; clamping shortens the late edge",
            timeout_symbols, max_symbols
        );
        RxMode::Single(max_symbols)
    }
}

fn compute_rx_window_timing(
    symbol_duration_us: u32,
    min_rx_symbols: u16,
    system_max_rx_error_ms: u32,
    wake_up_time_ms: u32,
) -> RxWindowTiming {
    let symbol_us = symbol_duration_us as u64;
    let min_symbols = min_rx_symbols.max(5) as u64;
    let error_us = system_max_rx_error_ms as u64 * 1_000;
    let lead_us = wake_up_time_ms as u64 * 1_000;

    // The radio is configured after the offset sleep, and the setup transaction
    // itself takes up to `wake_up_time_ms` (the lead time budget). We do not
    // know where in [0, lead] it actually lands, so the window has to cover the
    // nominal time for the whole range. Open `lead + error` early so the radio
    // is listening before the nominal window even when setup is slow and the
    // device clock runs fast, and keep the window open for `lead + 2 * error`
    // so it still spans the nominal time when setup is fast. This keeps the
    // timeout data-rate-aware (it scales with the symbol duration) without
    // assuming the setup latency equals the lead time, which is what made the
    // narrower windows close before the nominal time on radios with fast setup.
    let offset_us = -((lead_us + error_us) as i64);

    // Spanning the nominal time is not enough on its own: a packet arriving at
    // the late edge of the clock-error bound begins its preamble exactly as a
    // span-only timeout would fire, leaving no preamble symbols in the window
    // for the radio to detect. Extend the timeout by `min_symbols` so even the
    // latest packet has that much preamble in-window before the single-RX
    // timeout expires.
    let span_us = lead_us + 2 * error_us;
    let timeout_symbols = (span_us.div_ceil(symbol_us) + min_symbols).min(u16::MAX as u64) as u16;

    RxWindowTiming {
        // Truncation toward zero on a negative value matches ceil(offset_us /
        // 1000): the window opens no later than the microsecond-exact offset.
        offset_ms: (offset_us / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        timeout_symbols,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_timeout_scales_with_symbol_duration() {
        // The offset is set by the lead time and clock error (open lead + error
        // = 60 ms early), not the data rate. The timeout, in symbols, grows as
        // the symbols get shorter so the window spans the same wall-clock time,
        // plus min_symbols of preamble margin for a latest-edge arrival.

        // SF7/BW125: 1.024 ms per symbol. 70 ms span -> 69 symbols, +6 margin.
        assert_eq!(
            compute_rx_window_timing(1_024, 6, 10, 50),
            RxWindowTiming {
                offset_ms: -60,
                timeout_symbols: 75
            }
        );

        // SF12/BW125: 32.768 ms per symbol. 3 symbols cover the span, +6 margin.
        assert_eq!(
            compute_rx_window_timing(32_768, 6, 10, 50),
            RxWindowTiming {
                offset_ms: -60,
                timeout_symbols: 9
            }
        );
    }

    #[test]
    fn window_timeout_supports_faster_bandwidths() {
        assert_eq!(
            compute_rx_window_timing(512, 6, 10, 50),
            RxWindowTiming {
                offset_ms: -60,
                timeout_symbols: 143
            }
        );
        assert_eq!(
            compute_rx_window_timing(256, 6, 10, 50),
            RxWindowTiming {
                offset_ms: -60,
                timeout_symbols: 280
            }
        );
    }

    #[test]
    fn over_cap_window_falls_back_to_wall_clock_close() {
        // SF7/BW500 (256 us symbols), defaults: 280 symbols > the sx126x/lr11xx
        // 248-symbol ceiling. The wall-clock close covers the full span
        // (280 * 256 us = 71.68 ms, rounded up) instead of clamping to
        // 248 * 256 us = 63.5 ms, which would close before a latest-edge
        // packet inside the error bound arrives.
        let timing = compute_rx_window_timing(256, 6, 10, 50);
        assert_eq!(timing.timeout_symbols, 280);
        assert_eq!(select_single_rx_mode(280, 256, 248, true), RxMode::SingleMs(72));

        // lr11xx min_rx_symbols default of 13: 287 symbols, same fallback.
        let timing = compute_rx_window_timing(256, 13, 10, 50);
        assert_eq!(timing.timeout_symbols, 287);
        assert_eq!(select_single_rx_mode(287, 256, 248, true), RxMode::SingleMs(74));
    }

    #[test]
    fn under_cap_window_keeps_symbol_timeout() {
        // At or below the ceiling nothing changes: EU868's fastest window
        // (SF7/BW250 = 143+6 symbols) stays symbol-closed.
        assert_eq!(select_single_rx_mode(149, 512, 248, true), RxMode::Single(149));
        assert_eq!(select_single_rx_mode(248, 256, 248, true), RxMode::Single(248));
    }

    #[test]
    fn over_cap_window_clamps_without_wall_clock_timer() {
        // A chip with no wall-clock RX timer (sx127x) clamps to its ceiling;
        // only reachable there with an extreme configured error (its ceiling
        // is 1023 symbols).
        assert_eq!(select_single_rx_mode(1030, 1_024, 1023, false), RxMode::Single(1023));
    }

    #[test]
    fn larger_system_error_expands_and_shifts_window_earlier() {
        // error 50 ms, lead 50 ms: open 100 ms early, span 150 ms -> 147
        // symbols, +6 margin.
        assert_eq!(
            compute_rx_window_timing(1_024, 6, 50, 50),
            RxWindowTiming {
                offset_ms: -100,
                timeout_symbols: 153
            }
        );
    }
}
