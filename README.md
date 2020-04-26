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

## Benchmarks

Ran on Intel i7-8550U CPU @ 1.80GHz with 16GB RAM running Ubuntu 18.04.

* Benchmarks brocaar/lorawan (the code for the benchmarks can be found
  [here][3], results were obtained by running `go test -bench . -benchtime=5s`,
  `go1.13.1`)

```
pkg: github.com/brocaar/lorawan
BenchmarkDecode-8                  40410            150498 ns/op
BenchmarkValidateMic-8              2959           2026736 ns/op
BenchmarkDecrypt-8                  9390            648402 ns/op
```

* Benchmarks rust-lorawan (the code is inside `benches/lorawan.rs`, results are
  obtained running `cargo bench --workspace`, `rustc 1.43.0`)

```
  Running target/release/deps/lorawan-32e80b41705c7d41
Gnuplot not found, using plotters backend

data_payload_headers_parsing
      time:   [33.623 ns 33.670 ns 33.717 ns]
      change: [-0.2772% -0.0100% +0.2129%] (p = 0.93 > 0.05)
      No change in performance detected.
Found 7 outliers among 100 measurements (7.00%)
  5 (5.00%) low mild
  2 (2.00%) high mild

Approximate memory usage per iteration: 1 from 284778427

data_payload_mic_validation
      time:   [3.2744 us 3.2773 us 3.2799 us]
      change: [-0.2880% +0.1842% +0.5481%] (p = 0.44 > 0.05)
      No change in performance detected.
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild

Approximate memory usage per iteration: 191 from 2588825

data_payload_decrypt
      time:   [2.0159 us 2.0197 us 2.0249 us]
      change: [-4.9391% -4.6532% -4.2587%] (p = 0.00 < 0.05)
      Performance has improved.
Found 5 outliers among 100 measurements (5.00%)
  1 (1.00%) low mild
  1 (1.00%) high mild
  3 (3.00%) high severe

Approximate memory usage per iteration: 108 from 4576701
```

## Contributing

Please read [the contributing guidelines](CONTRIBUTING.md)

## Used code and inspiration

I would like to thank the projects [lorawan][1] by [brocaar][2] for the
inspiration and useful examples.

[1]: https://github.com/brocaar/lorawan
[2]: https://github.com/brocaar
[3]: https://gist.github.com/ivajloip/d63981e4caddaa68bd0b9c2390f4af90
[Latest Version]: https://img.shields.io/crates/v/lorawan.svg
[crates.io]: https://crates.io/crates/lorawan
[Docs]: https://docs.rs/lorawan/badge.svg
[doc.rs]: https://docs.rs/lorawan
