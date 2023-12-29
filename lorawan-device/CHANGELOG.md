# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to [Semantic Versioning](https://semver.org/).

## [v1.0.0] - Unreleased

- Fixes bug related to FCntUp and confirmed uplink ([#182](https://github.com/lora-rs/lora-rs/pull/182))
- Extend PhyRxTx to support antenna gain and max power ([#159](https://github.com/lora-rs/lora-rs/pull/159))
- Implement Class C functionality for async_device ([#158](https://github.com/lora-rs/lora-rs/pull/159))
- Implement rapid subband acquisition, aka "Join Bias" for US915 & AU915
  ([#110](https://github.com/lora-rs/lora-rs/pull/110) / [#170](https://github.com/lora-rs/lora-rs/pull/170) )
- Develops `async_device` API to provide `JoinResponse` and `SendResponse` (#[144](https://github.com/lora-rs/lora-rs/pull/144))
- Develops `nb_device` API around sending a join to be consistent with  `async_device` (#[144](https://github.com/lora-rs/lora-rs/pull/144))

---
Change tracking starting at version 0.11.0.
