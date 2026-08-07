# Benchmarks

The code is in `benches/lorawan.rs`; run with `cargo bench -p lorawan`. All
three benchmarks work on the same 18-byte unconfirmed data uplink: parse it,
read the header fields, validate the MIC, or decrypt the FRMPayload. A
tracking allocator reports heap usage per iteration (zero for all three).

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
