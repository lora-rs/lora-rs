# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to [Semantic Versioning](https://semver.org/).

## [v0.12.1]

- Allow multilple RXC frames during RXC window ([#217](https://github.com/lora-rs/lora-rs/pull/217))
- Individually feature-gate all regions ([#216](https://github.com/lora-rs/lora-rs/pull/236))
- Fix log macro for error ([commit](https://github.com/lora-rs/lora-rs/pull/256/commits/99cb10b77baf0f1c51ae97b1830a80b4873864e1))

## [v0.12.0]

- Fixes bug related to FCntUp and confirmed uplink ([#182](https://github.com/lora-rs/lora-rs/pull/182))
- Extend PhyRxTx to support antenna gain and max power ([#159](https://github.com/lora-rs/lora-rs/pull/159))
- Implement Class C functionality for async_device ([#158](https://github.com/lora-rs/lora-rs/pull/159))
- Implement rapid subband acquisition, aka "Join Bias" for US915 & AU915
  ([#110](https://github.com/lora-rs/lora-rs/pull/110) / [#170](https://github.com/lora-rs/lora-rs/pull/170) )
- Develops `async_device` API to provide `JoinResponse` and `SendResponse` (#[144](https://github.com/lora-rs/lora-rs/pull/144))
- Develops `nb_device` API around sending a join to be consistent with  `async_device` (#[144](https://github.com/lora-rs/lora-rs/pull/144))
- Refactor `external-lora-phy` in `lorawan-device` as `lorawan-radio` in `lora-phy` ([#189](https://github.com/lora-rs/lora-rs/pull/189))
- Add `Timer` implementation based on embassy-time ([#171](https://github.com/lora-rs/lora-rs/pull/171))
- Use radio timeout for end of RX1 and RX2 windows; preamble detection cancels timeout ([#204](https://github.com/lora-rs/lora-rs/pull/204))
- Remove `async` feature-flag as async fn in traits is stable 

Change tracking starting at version 0.11.0.
