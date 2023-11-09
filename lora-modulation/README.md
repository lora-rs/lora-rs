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
let length = 12;
let params = BaseBandModulationParams::new(SpreadingFactor::_5, Bandwidth::_500KHz, CodingRate::_4_5);
let time_on_air = params.time_on_air_us(
    Some(8), // preamble
    true,    // explicit header
    length); // length of payload
```

[Latest Version]: https://img.shields.io/crates/v/lora-modulation.svg
[crates.io]: https://crates.io/crates/lora-modulation
[Docs]: https://docs.rs/lora-modulation/badge.svg
[doc.rs]: https://docs.rs/lora-modulation