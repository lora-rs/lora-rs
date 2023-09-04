use super::*;

use ::lora_phy::mod_params;

/// Convert the spreading factor for use in the external lora-phy crate
impl From<SpreadingFactor> for mod_params::SpreadingFactor {
    fn from(sf: SpreadingFactor) -> Self {
        match sf {
            SpreadingFactor::_5 => lora_phy::mod_params::SpreadingFactor::_5,
            SpreadingFactor::_6 => lora_phy::mod_params::SpreadingFactor::_6,
            SpreadingFactor::_7 => lora_phy::mod_params::SpreadingFactor::_7,
            SpreadingFactor::_8 => lora_phy::mod_params::SpreadingFactor::_8,
            SpreadingFactor::_9 => lora_phy::mod_params::SpreadingFactor::_9,
            SpreadingFactor::_10 => lora_phy::mod_params::SpreadingFactor::_10,
            SpreadingFactor::_11 => lora_phy::mod_params::SpreadingFactor::_11,
            SpreadingFactor::_12 => lora_phy::mod_params::SpreadingFactor::_12,
        }
    }
}

/// Convert the bandwidth for use in the external lora-phy crate
impl From<Bandwidth> for mod_params::Bandwidth {
    fn from(bw: Bandwidth) -> Self {
        match bw {
            Bandwidth::_125KHz => lora_phy::mod_params::Bandwidth::_125KHz,
            Bandwidth::_250KHz => lora_phy::mod_params::Bandwidth::_250KHz,
            Bandwidth::_500KHz => lora_phy::mod_params::Bandwidth::_500KHz,
        }
    }
}

/// Convert the coding rate for use in the external lora-phy crate
impl From<CodingRate> for mod_params::CodingRate {
    fn from(cr: CodingRate) -> Self {
        match cr {
            CodingRate::_4_5 => lora_phy::mod_params::CodingRate::_4_5,
            CodingRate::_4_6 => lora_phy::mod_params::CodingRate::_4_6,
            CodingRate::_4_7 => lora_phy::mod_params::CodingRate::_4_7,
            CodingRate::_4_8 => lora_phy::mod_params::CodingRate::_4_8,
        }
    }
}
