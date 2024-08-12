# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to [Semantic Versioning](https://semver.org/).

## [v0.9.0]
- for AppEui, DevEui, AppKey: implement `core::str::FromStr`  (#[nostd] compatible) and
`std::str::ToString` (requires `with-to-string` feature and std) ([#234](https://github.com/lora-rs/lora-rs/pull/234))
- simplify features by removing `with-downlink`, as it has no impact on dependencies and
little impact on compilation time
- improvement to docs

## [v0.8.0]

- Add `packet_length` module containing constants for packet component sizes.
- update AES and CMAC libraries ([#190](https://github.com/lora-rs/lora-rs/pull/190))
- MacCommandCreator enhancements with add ADR fields ([#194](https://github.com/lora-rs/lora-rs/pull/194))
- Split MacCommands into Uplink and Dowlinks ([#178](https://github.com/lora-rs/lora-rs/pull/178)
- Specify AppKey, NewSKey, AppSKey in API instead of generic AES128 ([#177](https://github.com/lora-rs/lora-rs/pull/177)
- Use `enum Error` instead of `&str` for API's Result ([#175](https://github.com/lora-rs/lora-rs/pull/175) 

---

Change tracking starting at version 0.7.4.
