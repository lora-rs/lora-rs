# lorawan-device

[![Latest Version]][crates.io]
[![Docs]][doc.rs]

This is an experimental LoRaWAN device stack with both non-blocking (`nb_device`) and async (`async_device`) 
implementations. Both implementations have their respective `radio::PhyRxTx` traits that describe the radio interface
required. 

Note: The `external-lora-phy` feature provides the `lora-phy` crate as an async implementation of `radio::PhyRxTx`. 

LoRaWAN 1.0.x is approximately implemented:

- Class A device behavior
- Class C device behavior (async only)
- Over-the-Air Activation (OTAA) and Activation by Personalization (ABP)
- Regional support for AS923_1, AS923_2, AS923_3, AS923_4, AU915, EU868, EU433, IN865, US915
- CFList is supported for fixed and dynamic channel plans
- the stack starts deriving a new session when the FCnt maxes out the 32-bit
  counter; new session may also be created by any time by the user, as long the
  stack is not mid-transmit

**Currently, MAC commands are minimally mocked. For example, an ADRReq is responded with an ADRResp, but not much
is actually done with the payload**. 

Furthermore, both async and non-blocking implementation do not implement any retries for failed joins or failed 
confirmed uplinks. It is up to the client to implement retry behavior; see the examples for more.

Please see [examples](https://github.com/lora-rs/lora-rs/tree/main/examples) for usage.

[Latest Version]: https://img.shields.io/crates/v/lorawan-device.svg
[crates.io]: https://crates.io/crates/lorawan-device
[Docs]: https://docs.rs/lorawan-device/badge.svg
[doc.rs]: https://docs.rs/lorawan-device
