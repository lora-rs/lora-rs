# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to [Semantic Versioning](https://semver.org/).

## Unreleased

- Rewrite parser and creator as borrowed views, replacing the `T: AsRef<[u8]>`
  generic-over-storage design ([#465](https://github.com/lora-rs/lora-rs/pull/465)).
  Behavior changes from the previous API:
  - `f_port()` returns `Some` when an FPort byte is present with an empty
    FRMPayload; previously it returned `None`, conflating "FPort absent" with
    "FPort present, payload empty".
  - Fixed-size wire fields (`DevAddr`, `DevNonce`, etc.) convert to and from
    values little-endian, matching the wire; the previous `From<u32> for DevAddr`
    and `From<u16> for DevNonce` used big-endian array order, which round-tripped
    through the wire with flipped endianness.
  - All parse entry points reject frames with a non-R1 major version; the
    previous `EncryptedDataPayload::new` accepted them.
  - A MAC command stream with a malformed tail yields `Err` per command; the
    previous iterator ended silently. A lone `McGroupStatusAns` CID byte is
    reported as `Truncated`; the previous iterator panicked on that
    remotely-reachable input.
- Remove defmt feature from defaults, rename to defmt-03
- Mark `NewSKey` deprecated in favor of `NwkSkey` which is used in most LoRaWAN documentation.

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
