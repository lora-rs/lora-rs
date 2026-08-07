# LoRaWAN

[![Latest Version]][crates.io]
[![Docs]][doc.rs]

The lorawan library provides structures and tools to read and write LoRaWAN
packets from and to slices of bytes.

Supported LoRaWAN features:
* Class A (baseline) - up to 1.0.4 (1.1 unsupported)
* Class B (beacon) - unsupported
* Class C (continuous)
* Multicast - unsupported (only basic packet encoding/decoding)
* Relay - unsupported
* Certification - unsupported

## Sample Packet manipulation

### Use the library

```toml
[dependencies]
lorawan = "0.9"
```

### Packet generation

```rust
use lorawan::default_crypto::DefaultNetworkCrypto;
use lorawan::keys::AppKey;
use lorawan::types::DLSettings;
use lorawan::creator::JoinAccept;
use lorawan::parser::{CfList, DevAddr, Frequency, JoinNonce, NetId};

let accept = JoinAccept {
    join_nonce: JoinNonce::from_wire_bytes([1; 3]),
    net_id: NetId::from_wire_bytes([1; 3]),
    dev_addr: DevAddr::from_wire_bytes([1; 4]),
    dl_settings: DLSettings::new(2),
    rx_delay: 1,
    c_f_list: Some(CfList::DynamicChannel([
        Frequency::from_hz(867_100_000),
        Frequency::from_hz(867_300_000),
        Frequency::from_hz(867_500_000),
        Frequency::from_hz(867_700_000),
        Frequency::from_hz(867_900_000),
    ])),
};
let mut data = [0; 33];
let key = AppKey::from([1; 16]);
let crypto = DefaultNetworkCrypto::new(key.inner());
let payload = accept.build_into(&mut data, &crypto).unwrap();
println!("Payload: {:x?}", payload);
```

### Packet parsing

```rust
use lorawan::default_crypto::DefaultCrypto;
use lorawan::keys::AppSKey;
use lorawan::parser::{parse, DecryptedDataPayload, FrmPayload, PhyPayload};

let mut data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01,
0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6, 0xc3, 0xb5, 0x82];
// Inspect the frame read-only, then decrypt it in place.
let Ok(PhyPayload::Data(phy)) = parse(&data) else {
    panic!("failed to parse data payload");
};
let key = AppSKey::from([1; 16]);
let crypto = DefaultCrypto::new(key.inner());
let decrypted =
    DecryptedDataPayload::decrypt_in_place(&mut data, None, Some(&crypto), 1).unwrap();
if let FrmPayload::Data(data_payload) = decrypted.frm_payload() {
    println!("{}", String::from_utf8_lossy(data_payload));
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
