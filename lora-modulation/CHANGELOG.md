# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to [Semantic Versioning](https://semver.org/).

## Unreleased

- Move to Rust edition 2024 (requires Rust 1.85+)
- Add overflow-safe `delay_in_symbols_ceil` and expose `symbol_duration_us`
- Rename defmt feature to defmt-03
- Add `BaseBandModulationParams::low_data_rate_optimize` and derive LDRO from the (SF, BW) table Semtech's SWL2001 uses (`ral_compute_lora_ldro`) instead of a 16.384 ms symbol-time threshold

## [v0.1.5]
- Derive Eq for `Bandwidth`, `SpreadingFactor`, and `CodingRate`

## [v0.1.4]
- Add `BaseBandModulationParams::symbols_to_ms`

---

Change tracking starting at version 0.1.3.
