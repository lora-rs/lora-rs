# Flash-size harness for the borrowed-view parser API

`./run.sh` builds the workload (parse a frame, read header fields,
validate MIC, decrypt in place, fold the payload; plus the join paths)
against the borrowed-view parser API for `thumbv7em-none-eabi`, links it with
`--gc-sections`, and prints text sizes. A dummy `CryptoFactory` keeps AES
tables out of the measurement. Profile: `opt-level = "s"`,
`codegen-units = 1`; LTO is measured both ways.

Configurations:

* `new1`: one entry point (`&mut [u8]`).
* `new3`: three entry points (slice, owned `[u8; 33]`, wrapped array), each
  a thin shim over the same slice-based code.

The commit that introduced this API (as `v2`) kept the retired `T: AsRef<[u8]>` API
side-by-side and ran both through this harness (`old1`/`old3`
configurations). Results, 2026-08-03, rustc 1.97.1, all view accessors
`#[inline]`:

| cfg  | text bytes, lto=fat | text bytes, lto=off |
|------|--------------------:|--------------------:|
| old1 |                 874 |               5,595 |
| old3 |               1,872 |               6,059 |
| new1 |                 652 |               5,480 |
| new3 |               1,442 |               5,520 |

Takeaways:

* The new API is smaller in every configuration: −25% at a single storage type with
  LTO, −23% at three. Validate-once-in-`Layout` means accessors carry no
  repeated bounds checks for the optimizer to chew on.
* Every additional storage type costs the old API a further ~250 bytes
  (LTO) / ~230 bytes (no LTO) of duplicated parse/decrypt pipeline, already
  ICF-folded by the linker. The new API pays ~20-40 bytes for a shim.
* The `#[inline]` annotations on the parser's small accessors are load-bearing for
  the no-LTO column: without them the non-generic functions stay
  out-of-line across the crate boundary and new1 measures ~560 bytes WORSE
  than old1. Keep them if this ships.
