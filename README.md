# LoRaWAN

[![Build Status](https://travis-ci.org/ivajloip/rust-lorawan.svg?branch=master)](https://travis-ci.org/ivajloip/rust-lorawan)
[![Latest Version]][crates.io]
[![Docs]][doc.rs]
[![Gitter chat](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/rust-lorawan/lorawan)

Package lorawan provides structures and tools to read and write LoRaWAN
messages from and to a slice of bytes.

*NOTE*: This is a toy-project that I undertake in order to learn more about
Rust. Currrently it is far from stable or finished. Use at your own risk :)

## Already present

The following structures are implemented (+ fields):
* PhyPayload, MHDR, MType, Major
* MIC
* AES128
* MacPayload
* DataPayload
* EUI64
* DevNonce
* JoinRequestPayload
* JoinAcceptPayload
* DevAddr
* NwkAdr
* FHDR
* FCtrl
* FRMPayload
* FRMDataPayload
* FRMMacCommands
* MacCommand and creators for them
* LinkCkeckReq
* JoinRequestCreator
* JoinAcceptCreator
* DataPayloadCreator

MIC can be checked and FRMPayload and JoinAccept can be decrypted.

JoinRequest, JoinAccept and DataPayload packets can be constructed.

## Next steps

I plan to implement soon:

* [x] Finish with JoinAcceptPayload.
* [x] Add packet creation functions.
* [x] Finish with the mac commands.
* [ ] Add more tests.
* [ ] Check if there are any creators that I have forgotten.
* [ ] Calculate over the air time.

## Used code and inspiration

I would like to thank the projects [lorawan][1] by [brocaar][2] for the
inspiration and useful examples, [rust-crypto][3] by [DaGenix][4] for the AES
implentation and the form of rust-crypto by [a-dma][5] that helped me with the
implementation of cmac :)

[1]: https://github.com/brocaar/lorawan
[2]: https://github.com/brocaar
[3]: https://github.com/DaGenix/rust-crypto
[4]: https://github.com/DaGenix
[5]: https://github.com/a-dma
[Latest Version]: https://img.shields.io/crates/v/lorawan.svg
[crates.io]: https://crates.io/crates/lorawan
[Docs]: https://docs.rs/lorawan/badge.svg
[doc.rs]: https://docs.rs/lorawan
