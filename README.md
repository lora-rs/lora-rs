# lora-rs

[![Continuous Integration](https://github.com/lora-rs/lora-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/lora-rs/lora-rs/actions/workflows/rust.yml)

[![Matrix](https://img.shields.io/matrix/public-lora-wan-rs%3Amatrix.org)](https://matrix.to/#/#public-lora-wan-rs:matrix.org)

This repository aims to provide a set of compatible crates for implementing LoRa end devices in Rust. 
As a general rule, all crates are `nostd` and designed to be friendly for embedded projects.

## Crates

* **lora-modulation**: LoRa modulation characteristics and a utility for calculating time on air
* **lorawan-encoding**: encoding and decoding LoRaWAN packets
* **lorawan-device**: a LoRaWAN device stack with non-blocking and async implementations

## Contributing

Please read [the contributing guidelines](CONTRIBUTING.md).
