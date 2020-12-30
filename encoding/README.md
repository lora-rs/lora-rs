# LoRaWAN

[![Build Status](https://travis-ci.org/ivajloip/rust-lorawan.svg?branch=master)](https://travis-ci.org/ivajloip/rust-lorawan)
[![Latest Version]][crates.io]
[![Docs]][doc.rs]
[![Gitter chat](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/rust-lorawan/lorawan)

The lorawan library provides structures and tools for reading and writing
LoRaWAN 1.0.2 messages from and to slices of bytes.

## History of the crate

This crate was originially named lorawan and the original version can still be
found [here](https://crates.io/crates/lorawan). Due to the addition of lorawan
device stack crate, it was decided to be renamed to `lorawan-encoding`.

## Sample Packet manipulation

### Use the library

```toml
[dependencies]
lorawan = "0.6.2"
```

### Packet generation

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

* Benchmarks [brocaar/lorawan][4] (the code for the benchmarks can be found
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
      time:   [30.354 ns 30.430 ns 30.497 ns]
      change: [-5.5657% -5.1359% -4.7052%] (p = 0.00 < 0.05)
      Performance has improved.
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild

Approximate memory usage per iteration: 1 from 303847227

data_payload_mic_validation
      time:   [2.2334 us 2.2388 us 2.2476 us]
      change: [-3.7708% -3.3970% -2.8941%] (p = 0.00 < 0.05)
      Performance has improved.
Found 20 outliers among 100 measurements (20.00%)
  2 (2.00%) low severe
  5 (5.00%) low mild
  2 (2.00%) high mild
  11 (11.00%) high severe

Approximate memory usage per iteration: 114 from 4349451

data_payload_decrypt
      time:   [1.1179 us 1.1186 us 1.1193 us]
      change: [-0.8167% -0.4650% -0.1514%] (p = 0.00 < 0.05)
      Change within noise threshold.
Found 8 outliers among 100 measurements (8.00%)
  2 (2.00%) low severe
  2 (2.00%) low mild
  3 (3.00%) high mild
  1 (1.00%) high severe

Approximate memory usage per iteration: 57 from 8668603
```

[3]: https://gist.github.com/ivajloip/d63981e4caddaa68bd0b9c2390f4af90
[4]: https://github.com/brocaar/lorawan/commit/6095d473cf605ce4da4584ae2b570bca8e1259ff
[Latest Version]: https://img.shields.io/crates/v/lorawan.svg
[crates.io]: https://crates.io/crates/lorawan
[Docs]: https://docs.rs/lorawan/badge.svg
[doc.rs]: https://docs.rs/lorawan
