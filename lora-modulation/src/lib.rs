#![deny(rust_2018_idioms)]
#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Channel width. Lower values increase time on air, but may be able to find clear frequencies.
pub enum Bandwidth {
    _7KHz,
    _10KHz,
    _15KHz,
    _20KHz,
    _31KHz,
    _41KHz,
    _62KHz,
    _125KHz,
    _250KHz,
    _500KHz,
}

impl Bandwidth {
    pub const fn hz(self) -> u32 {
        match self {
            Bandwidth::_7KHz => 7810u32,
            Bandwidth::_10KHz => 10420u32,
            Bandwidth::_15KHz => 15630u32,
            Bandwidth::_20KHz => 20830u32,
            Bandwidth::_31KHz => 31250u32,
            Bandwidth::_41KHz => 41670u32,
            Bandwidth::_62KHz => 62500u32,
            Bandwidth::_125KHz => 125000u32,
            Bandwidth::_250KHz => 250000u32,
            Bandwidth::_500KHz => 500000u32,
        }
    }
}

impl From<Bandwidth> for u32 {
    fn from(value: Bandwidth) -> Self {
        value.hz()
    }
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Controls the chirp rate. Lower values are slower bandwidth (longer time on air), but more robust.
pub enum SpreadingFactor {
    _5,
    _6,
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
}

impl SpreadingFactor {
    pub const fn factor(self) -> u32 {
        match self {
            SpreadingFactor::_5 => 5,
            SpreadingFactor::_6 => 6,
            SpreadingFactor::_7 => 7,
            SpreadingFactor::_8 => 8,
            SpreadingFactor::_9 => 9,
            SpreadingFactor::_10 => 10,
            SpreadingFactor::_11 => 11,
            SpreadingFactor::_12 => 12,
        }
    }
}

impl From<SpreadingFactor> for u32 {
    fn from(sf: SpreadingFactor) -> Self {
        sf.factor()
    }
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Controls the forward error correction. Higher values are more robust, but reduces the ratio
/// of actual data in transmissions.
pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

impl CodingRate {
    pub const fn denom(&self) -> u32 {
        match self {
            CodingRate::_4_5 => 5,
            CodingRate::_4_6 => 6,
            CodingRate::_4_7 => 7,
            CodingRate::_4_8 => 8,
        }
    }
}

/// LoRa modulation parameters barring frequency
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseBandModulationParams {
    pub sf: SpreadingFactor,
    pub bw: Bandwidth,
    pub cr: CodingRate,
    /// low data rate optimization, see SX127x datasheet section 4.1.1.6
    pub ldro: bool,
    /// eagerly pre-calculated symbol duration in microseconds
    t_sym_us: u32,
}

impl BaseBandModulationParams {
    /// Create a set of parameters. Low data rate optimization is derived from
    /// `sf` and `bw`, see [`Self::low_data_rate_optimize`].
    pub const fn new(sf: SpreadingFactor, bw: Bandwidth, cr: CodingRate) -> Self {
        let t_sym_us = 2u32.pow(sf.factor()) * 1_000_000 / bw.hz();
        let ldro = Self::low_data_rate_optimize(sf, bw);
        Self { sf, bw, cr, ldro, t_sym_us }
    }

    /// Whether low data rate optimization (LDRO) is used for the given `sf` and `bw`.
    ///
    /// This is the table Semtech's reference stack uses for all of its LoRa
    /// transceivers (`ral_compute_lora_ldro` in SWL2001). The datasheets describe
    /// it as "symbol duration of 16.38 ms or longer", which is what the 62.5 kHz
    /// to 250 kHz rows follow; the narrower bandwidths turn it on for every SF
    /// instead. Both ends of a link must agree on this bit, so match the reference.
    pub const fn low_data_rate_optimize(sf: SpreadingFactor, bw: Bandwidth) -> bool {
        match bw {
            Bandwidth::_500KHz => false,
            Bandwidth::_250KHz => matches!(sf, SpreadingFactor::_12),
            Bandwidth::_125KHz => matches!(sf, SpreadingFactor::_11 | SpreadingFactor::_12),
            Bandwidth::_62KHz => {
                matches!(sf, SpreadingFactor::_10 | SpreadingFactor::_11 | SpreadingFactor::_12)
            }
            Bandwidth::_41KHz => matches!(
                sf,
                SpreadingFactor::_9
                    | SpreadingFactor::_10
                    | SpreadingFactor::_11
                    | SpreadingFactor::_12
            ),
            Bandwidth::_31KHz
            | Bandwidth::_20KHz
            | Bandwidth::_15KHz
            | Bandwidth::_10KHz
            | Bandwidth::_7KHz => true,
        }
    }

    /// Convert a millisecond duration to symbols, rounding up so the resulting
    /// symbol count never represents less than the requested duration.
    pub const fn delay_in_symbols_ceil(&self, delay_in_ms: u32) -> u16 {
        let delay_us = delay_in_ms as u64 * 1_000;
        let symbols = delay_us.div_ceil(self.t_sym_us as u64);
        if symbols > u16::MAX as u64 {
            u16::MAX
        } else {
            symbols as u16
        }
    }

    /// Duration of one symbol in microseconds.
    pub const fn symbol_duration_us(&self) -> u32 {
        self.t_sym_us
    }

    pub const fn symbols_to_ms(&self, symbols: u32) -> u32 {
        (self.t_sym_us * symbols) / 1_000
    }

    /// Calculates time on air for a given payload and modulation parameters.
    /// If `preamble` is None, the whole preamble including syncword is excluded from calculation.
    pub const fn time_on_air_us(
        &self,
        preamble: Option<u8>,
        explicit_header: bool,
        len: u8,
    ) -> u32 {
        let sf = self.sf.factor() as i32;
        let t_sym_us = self.t_sym_us;

        let cr = self.cr.denom() as i32;
        let de = if self.ldro {
            1
        } else {
            0
        };
        let h = if explicit_header {
            0
        } else {
            1
        };

        const fn div_ceil(num: i32, denom: i32) -> i32 {
            (num - 1) / denom + 1
        }

        let big_ratio = div_ceil(8 * len as i32 - 4 * sf + 28 + 16 - 20 * h, 4 * (sf - 2 * de));
        let big_ratio = if big_ratio > 0 {
            big_ratio
        } else {
            0
        };
        let payload_symb_nb = (8 + big_ratio * cr) as u32;

        match preamble {
            None => t_sym_us * payload_symb_nb,
            Some(preamble) => (4 * preamble as u32 + 17 + 4 * payload_symb_nb) * t_sym_us / 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LORAWAN_OVERHEAD: u8 = 13;
    // the shortest t_sym
    const SF5BW500: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_5, Bandwidth::_500KHz, CodingRate::_4_5);

    // EU868 DR6
    const SF7BW250: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_7, Bandwidth::_250KHz, CodingRate::_4_5);
    // EU868 DR5
    const SF7BW125: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_7, Bandwidth::_125KHz, CodingRate::_4_5);
    // EU868 DR4
    const SF8BW125: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_8, Bandwidth::_125KHz, CodingRate::_4_5);
    // EU868 DR3
    const SF9BW125: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_9, Bandwidth::_125KHz, CodingRate::_4_5);
    // EU868 DR2
    const SF10BW125: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_10, Bandwidth::_125KHz, CodingRate::_4_5);
    // EU868 DR1
    const SF11BW125: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_11, Bandwidth::_125KHz, CodingRate::_4_5);
    // EU868 DR0
    const SF12BW125: BaseBandModulationParams =
        BaseBandModulationParams::new(SpreadingFactor::_12, Bandwidth::_125KHz, CodingRate::_4_5);

    fn lorawan_airtime_us(params: &BaseBandModulationParams, app_payload_length: u8) -> u32 {
        params.time_on_air_us(Some(8), true, LORAWAN_OVERHEAD + app_payload_length)
    }

    // data for time-on-air tests is verified against:
    // * https://www.thethingsnetwork.org/airtime-calculator
    // * https://avbentem.github.io/airtime-calculator/ttn/

    #[test]
    fn time_on_air_for_short_messages() {
        assert_eq!(1152, SF5BW500.time_on_air_us(None, true, 0));
        assert_eq!(6656, SF7BW250.time_on_air_us(None, true, 0));
        assert_eq!(13312, SF7BW125.time_on_air_us(None, true, 0));
    }

    #[test]
    fn ldro_matches_semtech_table() {
        use Bandwidth::*;
        use SpreadingFactor::*;
        let bws =
            [_7KHz, _10KHz, _15KHz, _20KHz, _31KHz, _41KHz, _62KHz, _125KHz, _250KHz, _500KHz];
        // rows are SF5..SF12, columns follow `bws`
        let expected = [
            (_5, [1, 1, 1, 1, 1, 0, 0, 0, 0, 0]),
            (_6, [1, 1, 1, 1, 1, 0, 0, 0, 0, 0]),
            (_7, [1, 1, 1, 1, 1, 0, 0, 0, 0, 0]),
            (_8, [1, 1, 1, 1, 1, 0, 0, 0, 0, 0]),
            (_9, [1, 1, 1, 1, 1, 1, 0, 0, 0, 0]),
            (_10, [1, 1, 1, 1, 1, 1, 1, 0, 0, 0]),
            (_11, [1, 1, 1, 1, 1, 1, 1, 1, 0, 0]),
            (_12, [1, 1, 1, 1, 1, 1, 1, 1, 1, 0]),
        ];
        for (sf, row) in expected {
            for (bw, want) in bws.iter().zip(row) {
                let params = BaseBandModulationParams::new(sf, *bw, CodingRate::_4_5);
                assert_eq!(params.ldro, want == 1, "ldro for {sf:?}/{bw:?}");
                assert_eq!(BaseBandModulationParams::low_data_rate_optimize(sf, *bw), want == 1);
            }
        }
    }

    #[test]
    fn ldro_on_the_16_384ms_diagonal() {
        // Every cell on this diagonal has the same symbol duration (2^sf / bw
        // with sf + log2(500 kHz / bw) = 13), so LDRO must not depend on how
        // the bandwidth constants are rounded.
        use Bandwidth::*;
        use SpreadingFactor::*;
        for (sf, bw) in
            [(_7, _7KHz), (_8, _15KHz), (_9, _31KHz), (_10, _62KHz), (_11, _125KHz), (_12, _250KHz)]
        {
            let params = BaseBandModulationParams::new(sf, bw, CodingRate::_4_5);
            assert!(params.ldro, "ldro for {sf:?}/{bw:?}");
            assert!(params.symbol_duration_us().abs_diff(16_384) <= 8, "{sf:?}/{bw:?}");
        }
        assert_eq!(SF11BW125.symbol_duration_us(), 16_384);
    }

    #[test]
    fn delay_in_symbols_ceil_never_shortens_the_delay() {
        assert_eq!(1, SF7BW125.delay_in_symbols_ceil(1));
        assert_eq!(2, SF7BW125.delay_in_symbols_ceil(2));
        assert_eq!(u16::MAX, SF5BW500.delay_in_symbols_ceil(u32::MAX));
    }

    #[test]
    fn time_on_air() {
        let length = 25;
        assert_eq!(41_088, lorawan_airtime_us(&SF7BW250, length));
        assert_eq!(82_176, lorawan_airtime_us(&SF7BW125, length));
        assert_eq!(143_872, lorawan_airtime_us(&SF8BW125, length));
        assert_eq!(267_264, lorawan_airtime_us(&SF9BW125, length));
        assert_eq!(493_568, lorawan_airtime_us(&SF10BW125, length));
        assert_eq!(1_069_056, lorawan_airtime_us(&SF11BW125, length));
        assert_eq!(1_974_272, lorawan_airtime_us(&SF12BW125, length));

        let length = 26;
        assert_eq!(41_088, lorawan_airtime_us(&SF7BW250, length));
        assert_eq!(82_176, lorawan_airtime_us(&SF7BW125, length));
        assert_eq!(154_112, lorawan_airtime_us(&SF8BW125, length));
        assert_eq!(267_264, lorawan_airtime_us(&SF9BW125, length));
        assert_eq!(493_568, lorawan_airtime_us(&SF10BW125, length));
        assert_eq!(1_069_056, lorawan_airtime_us(&SF11BW125, length));
        assert_eq!(1_974_272, lorawan_airtime_us(&SF12BW125, length));

        let length = 27;
        assert_eq!(41_088, lorawan_airtime_us(&SF7BW250, length));
        assert_eq!(82_176, lorawan_airtime_us(&SF7BW125, length));
        assert_eq!(154_112, lorawan_airtime_us(&SF8BW125, length));
        assert_eq!(287_744, lorawan_airtime_us(&SF9BW125, length));
        assert_eq!(534_528, lorawan_airtime_us(&SF10BW125, length));
        assert_eq!(1_069_056, lorawan_airtime_us(&SF11BW125, length));
        assert_eq!(1_974_272, lorawan_airtime_us(&SF12BW125, length));

        let length = 28;
        assert_eq!(43_648, lorawan_airtime_us(&SF7BW250, length));
        assert_eq!(87_296, lorawan_airtime_us(&SF7BW125, length));
        assert_eq!(154_112, lorawan_airtime_us(&SF8BW125, length));
        assert_eq!(287_744, lorawan_airtime_us(&SF9BW125, length));
        assert_eq!(534_528, lorawan_airtime_us(&SF10BW125, length));
        assert_eq!(1_150_976, lorawan_airtime_us(&SF11BW125, length));
        assert_eq!(2_138_112, lorawan_airtime_us(&SF12BW125, length));
    }
}
