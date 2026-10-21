# lorawan-device

[![Latest Version]][crates.io]
[![Docs]][doc.rs]

This is an experimental LoRaWAN device stack with both non-blocking (`nb_device`) and async (`async_device`)
implementations. Both implementations have their respective `radio::PhyRxTx` traits that describe the radio interface
required.

Note: The `lorawan-radio` feature in the `lora-phy` crate provides `LorawanRadio` as an async implementation of
`radio::PhyRxTx`.

Both stacks share a dependency on the internal module, `mac` where LoRaWAN 1.0.x is approximately implemented:

- Class A device behavior
- Class C device behavior (async only, enabled by default with the `class-c` feature)
- `NbTrans` automatic uplink retransmission, controlled by the network via `LinkADRReq`
- Over-the-Air Activation (OTAA) and Activation by Personalization (ABP)
- CFList is supported for fixed and dynamic channel plans
- Regional support for AS923_1, AS923_2, AS923_3, AS923_4, AU915, EU868, EU433, IN865, RU864, US915 with following caveats:
  * FSK and LR-FHSS modulations are not supported

**Currently, not all MAC commands are fully implemented**. These commands
are gated behind the "experimental" feature.

Both stacks implement `NbTrans` automatic retransmission for uplinks. `NbTrans`
defaults to 1 transmission and is set by the network with a `LinkADRReq`
(LoRaWAN 1.0.4). With `NbTrans` > 1, an uplink that receives no downlink is
repeated with the same `FCntUp`, up to `NbTrans` times. Confirmed uplinks wait
`RETRANSMIT_TIMEOUT` before each repeat. After the last confirmed attempt, the
stack waits `RETRANSMIT_TIMEOUT` before reporting `NoAck`. In Class C the wait
is spent listening in receive-continuous mode, so downlinks are not missed. ADR
is enabled by default, which lets the network manage `NbTrans`. There is no
application API to set it directly.

Failed joins, and confirmed uplinks that report `NoAck` after all `NbTrans`
attempts, must still be retried by the application. See the examples for more.

Please see [examples](https://github.com/lora-rs/lora-rs/tree/main/examples) for usage.

A public chat on LoRa/LoRaWAN topics using Rust is [here](https://matrix.to/#/#public-lora-wan-rs:matrix.org).

[Latest Version]: https://img.shields.io/crates/v/lorawan-device.svg
[crates.io]: https://crates.io/crates/lorawan-device
[Docs]: https://docs.rs/lorawan-device/badge.svg
[doc.rs]: https://docs.rs/lorawan-device
