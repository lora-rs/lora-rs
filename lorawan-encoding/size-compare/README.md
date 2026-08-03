# Flash-size comparison: old generic API vs v2 borrowed views

`./run.sh` builds the same workload (parse a frame, read header fields,
validate MIC, decrypt in place, fold the payload; plus the join paths)
against the old `T: AsRef<[u8]>` API and the v2 borrowed-view API for
`thumbv7em-none-eabi`, links each with `--gc-sections`, and prints text
sizes. A dummy `CryptoFactory` keeps AES tables out of the measurement.
Profile: `opt-level = "s"`, `codegen-units = 1`; LTO is measured both ways.

Configurations:

* `old1` / `new1`: one storage type (`&mut [u8]`).
* `old3` / `new3`: three storage types (slice, owned `[u8; 33]`, newtype
  wrapper). With the old API each is a fresh monomorphization; with the new
  API each is a thin entry point over the same slice-based code.

Results, 2026-08-03, rustc 1.97.1, all v2 accessors `#[inline]`:

| cfg  | text bytes, lto=fat | text bytes, lto=off |
|------|--------------------:|--------------------:|
| old1 |                 874 |               5,595 |
| old3 |               1,872 |               6,059 |
| new1 |                 652 |               5,480 |
| new3 |               1,442 |               5,520 |

Takeaways:

* v2 is smaller in every configuration: −25% at a single storage type with
  LTO, −23% at three. Validate-once-in-`Layout` means accessors carry no
  repeated bounds checks for the optimizer to chew on.
* Every additional storage type costs the old API a further ~250 bytes
  (LTO) / ~230 bytes (no LTO) of duplicated parse/decrypt pipeline, already
  ICF-folded by the linker. The new API pays ~20-40 bytes for a shim.
* The `#[inline]` annotations on v2's small accessors are load-bearing for
  the no-LTO column: without them the non-generic functions stay
  out-of-line across the crate boundary and new1 measures ~560 bytes WORSE
  than old1. Keep them if this ships.
