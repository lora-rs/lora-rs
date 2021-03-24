use super::{Bandwidth, SpreadingFactor, Datarate};

pub(crate) const DATARATES: [Datarate; 5] = [
    Datarate { spreading_factor: SpreadingFactor::_12, bandwidth:  Bandwidth::_125KHZ},
    Datarate { spreading_factor: SpreadingFactor::_11, bandwidth:  Bandwidth::_125KHZ},
    Datarate { spreading_factor: SpreadingFactor::_10, bandwidth:  Bandwidth::_125KHZ},
    Datarate { spreading_factor: SpreadingFactor::_9,  bandwidth: Bandwidth::_125KHZ},
    Datarate { spreading_factor: SpreadingFactor::_8,  bandwidth: Bandwidth::_125KHZ},
];
