# LoRaWAN

[![Latest Version]][crates.io]
[![Docs]][doc.rs]

The lorawan library provides structures and tools to read and write LoRaWAN
packets from and to slices of bytes.

Supported LoRaWAN features:
* Class A (baseline) - up to 1.0.4 (1.1 unsupported)
* Class B (beacon) - unsupported
* Class C (continuous) - multicast unsupported
* Relay - unsupported

## Sample Packet manipulation

### Use the library

```toml
[dependencies]
lorawan = "0.9"
```

### Packet generation

```rust
use lorawan::{creator::JoinAcceptCreator, keys};
use lorawan::default_crypto::DefaultFactory;
use lorawan::types::Frequency;

fn main() {
    let mut data = [0; 33];
    let mut phy = JoinAcceptCreator::new(&mut data).unwrap();
    let key = keys::AES128([1; 16]);
    let app_nonce_bytes = [1; 3];
    phy.set_app_nonce(&app_nonce_bytes);
    phy.set_net_id(&[1; 3]);
    phy.set_dev_addr(&[1; 4]);
    phy.set_dl_settings(2);
    phy.set_rx_delay(1);
    let mut freqs = [
        Frequency::new(&[0x58, 0x6e, 0x84,]).unwrap(),
        Frequency::new(&[0x88, 0x66, 0x84,]).unwrap()
    ];
    phy.set_c_f_list(freqs).unwrap();
    let crypto_factory = DefaultFactory::default();
    let payload = phy.build(&key,&crypto_factory).unwrap();
    println!("Payload: {:x?}", payload);
}
```

### Packet parsing

```rust
use lorawan::parser::*;
use lorawan::keys::*;

fn main() {
    let data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01,
    0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6, 0xc3, 0xb5, 0x82];
    if let Ok(PhyPayload::Data(DataPayload::Encrypted(phy))) = parse(data) {
        let key = AES128([1; 16]);
        let decrypted = phy.decrypt(None, Some(&key), 1).unwrap();
        if let FRMPayload::Data(data_payload) = decrypted.frm_payload() {
                println!("{}", String::from_utf8_lossy(data_payload));
        }
    } else {
        panic!("failed to parse data payload");
    }
}
```

## Benchmarking

Run `cargo bench` and see `benches` directory.

## Used code and inspiration

Code in this repository has been inspired by [lorawan][5] project by [brocaar][6].

[5]: https://github.com/brocaar/lorawan
[6]: https://github.com/brocaar
[Latest Version]: https://img.shields.io/crates/v/lorawan.svg
[crates.io]: https://crates.io/crates/lorawan
[Docs]: https://docs.rs/lorawan/badge.svg
[doc.rs]: https://docs.rs/lorawan
