# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## [v3.0.0] - Unreleased

- Switch SpiInterface to use SpiDevice trait.
- Implement functionality for continuous wave mode.
- Support preamble detection allowing LoRaWAN RX1+RX2 reception.
- Update embedded-hal-async version to 1.0.0.
- Refactor `prepare_for_rx` to use RxMode enum and drop now unneeded args
- Refactor `external-lora-phy` in `lorawan-device` as `lorawan-radio` in `lora-phy` ([#189](https://github.com/lora-rs/lora-rs/pull/189))
- Refactor `setup_rx` to take in `enum RxMode` and drop previous `timeout_in_seconds` argument ([#207](https://github.com/lora-rs/lora-rs/pull/207))
- Refactor `do_rx` and lorawan-radio's `setup_for_rx`[#208](https://github.com/lora-rs/lora-rs/pull/208)
- Refactor sx126x power amplifier control and clarify `Sx126xVariant` implications ([#209](https://github.com/lora-rs/lora-rs/pull/209))
- Drop general AsyncRng trait and its implementation in sx126x driver ([#222](https://github.com/lora-rs/lora-rs/pull/227))
- Drop IRQ polling from `process_irq` and rely on IRQ pins exclusively ([#223](https://github.com/lora-rs/lora-rs/pull/223))
- sx126x: recalibrate when TCXO enabled ([#228](https://github.com/lora-rs/lora-rs/pull/228))
- Simplify RadioKind traits ([#229](https://github.com/lora-rs/lora-rs/pull/229))
- Add `set_standby` to public `LoRa` interface ([#230](https://github.com/lora-rs/lora-rs/pull/230))
- Refactor IRQ handling such that `wait_for_irq` is droppable (ie: for use in select branches) ([#231](https://github.com/lora-rs/lora-rs/pull/231))
- Refactor internal Rx state to use `RxMode` enum ([#242](https://github.com/lora-rs/lora-rs/pull/242))
- Modify `prepare_for_rx` and `rx` to enabled tighter timings ([#245](https://github.com/lora-rs/lora-rs/pull/245))
- Remove `timeout_in_ms` from `tx` ([#246](https://github.com/lora-rs/lora-rs/pull/246))
- Allow `process_irq_event` to continue on RFU ([#247](https://github.com/lora-rs/lora-rs/pull/247))
- Make Sx126x ([#249](https://github.com/lora-rs/lora-rs/pull/249)) and Sx127x ([#248](https://github.com/lora-rs/lora-rs/pull/248)) variants trait based rather than enum 
- Call `wait_for_irq` before `process_irq_event` in `LoRa::cad` ([#250](https://github.com/lora-rs/lora-rs/pull/250))

## [v2.1.2] - 2023-09-25

### Changed
- Minor README update.

## [v2.1.1] - 2023-09-14

### Changed
- Use the lora-modulation crate for modulation enums.
- Update dependencies.
- Make embedded_hal_async::delay::DelayUs available publically to avoid a dependency in crates using lora-phy.

## [v2.1.0] - 2023-07-07

### Changed
- Update nightly version.
- Update embedded-hal-async version.

## [v2.0.0] - 2023-06-25

### Changed
- Implement lora-phy API changes, requiring a new major version.
- For receive single packet, poll for interrupts to support LoRa chips that would require more than one DIO pin to support timeout IRQs.
- For receive single packet, depend on symbol timeout to prevent window duration timeouts from voiding reception of a packet that needs additional time to be received. This is useful for LoRaWAN Rx1/Rx2 windowing.
- Improve cold start after sleep processing, to prolong battery life between transmissions.
- Provide further flexibility to support custom boards through proprietary RadioKind implementations.

## [v1.2.0] - 2023-06-01

### Added
- Add support for the RAK3172 LoRa board.
- Allow custom radio kind implementations for LoRa boards based on sx1261/2 or sx1276/7/8/9.

### Changed
- Remove unnecessary static trait bounds.
- Change read status error handling on IRQ flags to ensure actual Rx timeout flags are cleared appropriately.

## [v1.1.0] - 2023-05-14

### Added
- Random number generation for LoRa boards which support it.

## [v1.0.2] - 2023-04-26

### Added
- .vscode settings.

### Changed
- README to reflect merges into the base rust-lorawan and embassy repositories.
- formatting.

## [v1.0.1] - 2023-04-21

### Changed
- `embedded-hal-async` version.
- formatting.

## [v1.0.0] - 2023-04-14

- first release to crates.io.


[Unreleased]: https://github.com/embassy-rs/lora-phy/compare/v2.1.2...HEAD
[v2.1.2]: https://github.com/embassy-rs/lora-phy/compare/v2.1.1...v2.1.2
[v2.1.1]: https://github.com/embassy-rs/lora-phy/compare/v2.1.0...v2.1.1
[v2.1.0]: https://github.com/embassy-rs/lora-phy/compare/v2.0.0...v2.1.0
[v2.0.0]: https://github.com/embassy-rs/lora-phy/compare/v1.2.0...v2.0.0
[v1.2.0]: https://github.com/embassy-rs/lora-phy/compare/v1.1.0...v1.2.0
[v1.1.0]: https://github.com/embassy-rs/lora-phy/compare/v1.0.2...v1.1.0
[v1.0.2]: https://github.com/embassy-rs/lora-phy/compare/v1.0.1...v1.0.2
[v1.0.1]: https://github.com/embassy-rs/lora-phy/compare/v1.0.0...v1.0.1
[v1.0.0]: https://github.com/embassy-rs/lora-phy/tree/v1.0.0
