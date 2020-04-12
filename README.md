# LoRaWAN

[![Build Status](https://travis-ci.org/ivajloip/rust-lorawan.svg?branch=master)](https://travis-ci.org/ivajloip/rust-lorawan)
[![Latest Version]][crates.io]
[![Docs]][doc.rs]
[![Gitter chat](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/rust-lorawan/lorawan)

The lorawan library provides structures and tools for reading and writing
LoRaWAN 1.0.2 messages from and to slices of bytes.

# Sample Packet manipulation

## Use the library

```toml
[dependencies]
lorawan = "0.5.0"
```

## Packet generation

```rust
use lorawan::{creator, keys, maccommands};
use heapless;

fn main() {
    let mut phy = creator::JoinAcceptCreator::new();
    let key = keys::AES128([1; 16]);
    let app_nonce_bytes = [1; 3];
    phy.set_app_nonce(&app_nonce_bytes);
    phy.set_net_id(&[1; 3]);
    phy.set_dev_addr(&[1; 4]);
    phy.set_dl_settings(2);
    phy.set_rx_delay(1);
    let mut freqs: heapless::Vec<lorawan::maccommands::Frequency, heapless::consts::U256> = heapless::Vec::new();
    freqs.push(maccommands::Frequency::new(&[0x58, 0x6e, 0x84,]).unwrap()).unwrap();
    freqs.push(maccommands::Frequency::new(&[0x88, 0x66, 0x84,]).unwrap()).unwrap();
    phy.set_c_f_list(freqs).unwrap();
    let payload = phy.build(&key).unwrap();
    println!("Payload: {:x?}", payload);
}
```

## Packet parsing

```rust
use lorawan::parser::*;
use lorawan::keys::*;

fn main() {
    let data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01,
    0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6, 0xc3, 0xb5, 0x82];
    if let Ok(PhyPayload::Data(DataPayload::Encrypted(phy))) = parse(data) {
        let key = AES128([1; 16]);
        let decrypted = phy.decrypt(None, Some(&key), 1).unwrap();
        if let Ok(FRMPayload::Data(data_payload)) = decrypted.frm_payload() {
                println!("{}", String::from_utf8_lossy(data_payload));
        }
    } else {
        panic!("failed to parse data payload");
    }
}
```

## Contributing

Please read [the contributing guidelines](CONTRIBUTING.md)

## Used code and inspiration

I would like to thank the projects [lorawan][1] by [brocaar][2] for the
inspiration and useful examples.

[1]: https://github.com/brocaar/lorawan
[2]: https://github.com/brocaar
[Latest Version]: https://img.shields.io/crates/v/lorawan.svg
[crates.io]: https://crates.io/crates/lorawan
[Docs]: https://docs.rs/lorawan/badge.svg
[doc.rs]: https://docs.rs/lorawan
