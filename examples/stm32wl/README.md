# stm32wl examples

## Overview

These examples demonstrate how to use lora-phy (and sometimes lorawan-device) on the STM32WL platform.
The NUCLEO-WL55JC1 board is a readily available development board for the STM32WL55JC SoC and it features an on-board debugger.

## Building and running

Build these examples with the release profile. Unoptimized development builds are
substantially larger and slower on this constrained target, and their different
timing can prevent the radio examples from operating correctly.

For example, to run the peer-to-peer transmitter from this directory:

```sh
cargo run --release --bin lora_p2p_send
```

Replace `lora_p2p_send` with any of the other binaries in `src/bin` as needed.
The release profile retains debug information, so `probe-rs` can still report
symbolized backtraces and source locations.

Running these examples without `--release` is not supported.
