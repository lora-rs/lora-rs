# Benchmarks

The code is in `benches/lorawan.rs`; run with `cargo bench -p lorawan`. The
benchmarks work on the same 18-byte unconfirmed data uplink: parse it, read
the header fields, validate the MIC, decrypt the FRMPayload, or build the
frame from scratch. A tracking allocator reports heap usage per iteration
(zero for all of them).

## Reference numbers

Measured on an AMD EPYC 7302 (AES-NI) with `rustc 1.97.1`, crypto bound to
the session keys outside the loop (`DefaultCrypto` caches the AES key
schedule per key):

| benchmark                    |     time |
|------------------------------|---------:|
| data_payload_headers_parsing |  6.46 ns |
| data_payload_mic_validation  | 116.8 ns |
| data_payload_decrypt         |  41.1 ns |
| data_payload_creation        | 173.6 ns |

## Comparison against ChirpStack's lrwn crate

[`lrwn`](https://github.com/chirpstack/chirpstack/tree/master/lrwn)
(v4.20.0-test.2) is ChirpStack's Rust LoRaWAN framing library; it decodes
frames into owned structs. Measured with a separate harness running both
crates on the same vector on an AMD EPYC 7302 with `rustc 1.97.1`, inputs
and outputs wrapped in `black_box`:

| operation | lorawan | lrwn |
|---|--:|--:|
| parse + read header fields | 14.9 ns | 83.0 ns |
| MIC validation | 152 ns | 508 ns |
| decrypt FRMPayload | 70.8 ns | 210 ns |

The gap is structural rather than a code-quality difference: `lrwn`
allocates owned structures per frame and re-serializes the payload for MIC
computation, both fine trade-offs for a network server, while this crate
validates once and reads through borrowed views.
