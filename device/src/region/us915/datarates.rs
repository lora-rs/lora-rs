use super::{Datarate, Bandwidth, SpreadingFactor};

pub(crate) const DATARATES: [Option<Datarate>; 14] = [
    Some(Datarate { spreading_factor: SpreadingFactor::_10, bandwidth:  Bandwidth::_125KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_9,  bandwidth: Bandwidth::_125KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_8,  bandwidth: Bandwidth::_125KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_7,  bandwidth: Bandwidth::_125KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_8,  bandwidth: Bandwidth::_500KHZ}),
    None,
    None,
    None,
    Some(Datarate { spreading_factor: SpreadingFactor::_12, bandwidth:  Bandwidth::_500KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_11, bandwidth:  Bandwidth::_500KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_10, bandwidth:  Bandwidth::_500KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_9,  bandwidth: Bandwidth::_500KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_8,  bandwidth: Bandwidth::_500KHZ}),
    Some(Datarate { spreading_factor: SpreadingFactor::_7,  bandwidth: Bandwidth::_500KHZ}),
];