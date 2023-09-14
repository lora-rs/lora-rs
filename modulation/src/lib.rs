#![no_std]

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
/// Channel width.
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

impl From<Bandwidth> for u32 {
    fn from(value: Bandwidth) -> Self {
        match value {
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
/// Controls the chirp rate. Lower values are slower bandwidth, but more robust.
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

impl From<SpreadingFactor> for u32 {
    fn from(sf: SpreadingFactor) -> Self {
        match sf {
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
/// Controls the forward error correction. Higher values are more robust, but reduces the ratio
/// of actual data in transmissions.
pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}
