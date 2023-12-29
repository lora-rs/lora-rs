# LoRa physical layer (the rustaceous radio)

[![CI](https://github.com/lora-rs/lora-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/lora-rs/lora-rs/actions/workflows/rust.yml)

## Why?

- provide one straight-forward LoRa physical layer API which supports both LoRaWAN and point-to-point (P2P) use cases;
- support a variety of microcontroller unit (MCU) and LoRa chip combinations behind one API;
- enable LoRa features for any embedded framework which supports <a href="https://github.com/rust-embedded/embedded-hal/tree/master/embedded-hal-async/src/">embedded-hal-async</a> for the desired MCU/LoRa chip combination.

## How?

- separate out modulation parameters and packet parameters as separate concerns to address the nuances in LoRa chip support and to allow flexible specification of various LoRaWAN and P2P send/receive channels, even in the same use case;
- allow the user to specify a LoRa chip kind (for example, Sx1261/2) and specific LoRa board type (for example, Stm32wlSx1262) and hide the control of that LoRa board behind the LoRa physical layer API;
- provide a minimal trait which must be implemented for each desired embedded framework/MCU type/LoRa chip type to allow this crate to interface to the LoRa chip within the embedded framework.

## Wheretofore?

- while the current examples use the Embassy embedded framework, nrf52840, rp pico, stm32l0, and stm32wl MCUs, and Sx127x/Sx126x chips, this crate provides a path forward for other embedded frameworks, MCU types, and LoRa chips in a Rust development environment.

## Examples

Please see [examples](https://github.com/lora-rs/lora-rs/tree/main/examples) for usage.

## Chat

A public chat on LoRa/LoRaWAN topics using Rust is here:

- <a href="https://matrix.to/#/#public-lora-wan-rs:matrix.org">Matrix room</a>

## LoRa physical layer API

For users wishing to implement a LoRaWAN or P2P solution, the following implementation files provide the necessary context for lora-phy version 2.

- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/lib.rs">the API itself</a>;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/mod_params.rs">pertinent ancillary information</a>.

Examples of API usage:

- <a href="https://github.com/lora-rs/lora-rs/blob/main/examples/stm32wl/src/bin/lora_p2p_send.rs">stm32wl P2P send and sleep</a>;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/examples/stm32wl/src/bin/lora_lorawan.rs">stm32wl LoRaWAN using rust-lorawan</a>;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/examples/stm32l0/src/bin/lora_p2p_receive.rs">stm32l0 P2P receive continuous</a>;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/examples/nrf52840/src/bin/lora_p2p_receive_duty_cycle.rs">nrf52840 duty cycle receive</a>;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/examples/nrf52840/src/bin/lora_cad.rs">nrf52840 channel activity detection</a>;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/examples/rp/src/bin/lora_p2p_send_multicore.rs">rp pico P2P send and sleep using the second core</a>.

## Embedded framework/MCU support

For embedded framework developers wishing to add LoRa support as a feature for one or more MCU/LoRa chip combinations:

- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/mod_traits.rs">the InterfaceVariant trait</a>, which enables this lora-phy crate to interface to a specific embedded framework/MCU/LoRa chip combination.

Example InterfaceVariant implementations:

- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/iv.rs">Implementations based on `embedded-hal` and `embedded-hal-async` traits</a>. These are usable with any HAL crate that implements the `embedded-hal` traits.
- <a href="https://github.com/lora-rs/lora-rs/blob/main/examples/stm32wl/src/iv.rs">STM32WL + Embassy implementation</a>. STM32WL is special because LoRa uses an internal SPI, this implementation shows how to use it with `embassy-stm32`.

## LoRa chip support

For developers wishing to add support for new LoRa chips or enhance support for existing chips:

- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/mod_traits.rs">the RadioKind trait</a>, which must be implemented for each kind of LoRa chip for access through the lora-phy crate API;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/interface.rs">the interface implementation</a>, which captures the three key read/write operations allowing control of the LoRa chip from this crate through either opcode or register operations.

Example RadioKind implementations and ancillary information:

- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/sx1261_2">the Sx1261/2 radio kind</a>;
- <a href="https://github.com/lora-rs/lora-rs/blob/main/lora-phy/src/sx1276_7_8_9">the Sx1276/7/8/9 radio kind</a>.

## LoRa board-specific support

Board-specific configuration can be handled via the chip driver specific Config struct.

