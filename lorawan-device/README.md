# lorawan-device

[![Latest Version]][crates.io]
[![Docs]][doc.rs]

This is an experimental LoRaWAN device stack with both non-blocking (`nb_device`) and async (`async_device`)
implementations. Both implementations have their respective `radio::PhyRxTx` traits that describe the radio interface
required.

Note: The `lorawan-radio` feature in the `lora-phy` crate provides `LorawanRadio` as an async implementation of
`radio::PhyRxTx`.

Both stacks share a dependency on the internal module, `mac` where LoRaWAN 1.0.4 is approximately implemented.
The regional parameter implementation currently targets RP002 1.0.3.

- Class A device behavior
- Class C device behavior (async only, available with the `class-c` feature)
- `NbTrans` automatic uplink retransmission, controlled by the network via `LinkADRReq`
- ADR is supported and enabled by default
- Over-the-Air Activation (OTAA) and Activation by Personalization (ABP)
- CFList is supported for fixed and dynamic channel plans
- Regional support for AS923_1, AS923_2, AS923_3, AS923_4, AU915, EU868, EU433, IN865, RU864, US915 with following caveats:
  * FSK and LR-FHSS modulations are not supported

**Currently, not all MAC commands are fully implemented**. These commands
are gated behind the "experimental" feature.

Both stacks implement `NbTrans` automatic retransmission for uplinks. `NbTrans`
defaults to 1 transmission and is managed by the network with `LinkADRReq`.
With `NbTrans` > 1, an uplink that receives no downlink is repeated with the
same `FCntUp`, up to `NbTrans` transmissions. Confirmed uplinks wait
`RETRANSMIT_TIMEOUT` (random delay between 1-3 seconds) before each repeat and
before reporting `NoAck` after the last attempt. For Class C, the wait is spent
listening in continuous receive mode.

Failed joins, and confirmed uplinks that report `NoAck` after all `NbTrans`
attempts, must still be retried by the application.

When pending MAC answers do not fit in FOpts, they are sent automatically in
FPort 0 and the application payload is dropped. Call
`device.take_payload_dropped()` after `device.send()` to handle these cases if
required:

```rust,ignore
let payload = [..];
let response = device.send(&payload, 1, false).await?;

if device.take_payload_dropped() {
    // Handle the case where application data was dropped.
}
```

See the examples for more.

Please see [examples](https://github.com/lora-rs/lora-rs/tree/main/examples) for usage.

A public chat on LoRa/LoRaWAN topics using Rust is [here](https://matrix.to/#/#public-lora-wan-rs:matrix.org).

[Latest Version]: https://img.shields.io/crates/v/lorawan-device.svg
[crates.io]: https://crates.io/crates/lorawan-device
[Docs]: https://docs.rs/lorawan-device/badge.svg
[doc.rs]: https://docs.rs/lorawan-device
