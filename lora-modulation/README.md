# lora-modulation

[![Latest Version]][crates.io]
[![Docs]][doc.rs]

A minimal crate for providing LoRa modulation characteristics of:
* Bandwidth
* Spreading factor
* Coding rate

Provides utility for calculating time on air.

## Usage

```rust
use lora_modulation::{BaseBandModulationParams, SpreadingFactor, Bandwidth, CodingRate};

let length = 12;
let params = BaseBandModulationParams::new(SpreadingFactor::_9, Bandwidth::_125KHz, CodingRate::_4_5);
let time_on_air = params.time_on_air_us(
    Some(8), // preamble
    true,    // explicit header
    length); // length of payload

// Time on air is 144.384 ms
assert_eq!(time_on_air, 144384);
```

```rust
use lora_modulation::{BaseBandModulationParams, SpreadingFactor, Bandwidth, CodingRate};

let symbols = 14;
let params = BaseBandModulationParams::new(SpreadingFactor::_12, Bandwidth::_125KHz, CodingRate::_4_5);
let timeout = params.symbols_to_ms(symbols);

// Timeout is 458 ms
assert_eq!(timeout, 458);
```

[Latest Version]: https://img.shields.io/crates/v/lora-modulation.svg
[crates.io]: https://crates.io/crates/lora-modulation
[Docs]: https://docs.rs/lora-modulation/badge.svg
[doc.rs]: https://docs.rs/lora-modulation
