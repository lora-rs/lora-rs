# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

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


[Unreleased]: https://github.com/embassy-rs/lora-phy/compare/v2.1.0...HEAD
[v2.1.0]: https://github.com/embassy-rs/lora-phy/compare/v2.0.0...v2.1.0
[v2.0.0]: https://github.com/embassy-rs/lora-phy/compare/v1.2.0...v2.0.0
[v1.2.0]: https://github.com/embassy-rs/lora-phy/compare/v1.1.0...v1.2.0
[v1.1.0]: https://github.com/embassy-rs/lora-phy/compare/v1.0.2...v1.1.0
[v1.0.2]: https://github.com/embassy-rs/lora-phy/compare/v1.0.1...v1.0.2
[v1.0.1]: https://github.com/embassy-rs/lora-phy/compare/v1.0.0...v1.0.1
[v1.0.0]: https://github.com/embassy-rs/lora-phy/tree/v1.0.0
