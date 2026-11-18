use crate::sx128x::radio_kind_params::Sx128xVariant;

/// SX1281: same as the SX1280 without the ranging engine
pub struct Sx1281;

impl Sx128xVariant for Sx1281 {}
