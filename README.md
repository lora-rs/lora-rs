# LoRa physical layer (the rustaceous radio)

## Why?

- provide one straight-forward LoRa physical layer API which supports both LoRaWAN and point-to-point (P2P) use cases;
- support a variety of microcontroller unit (MCU) and LoRa chip combinations behind one API;
- enable LoRa features for any embedded framework which supports <a href="https://github.com/rust-embedded/embedded-hal/tree/master/embedded-hal-async/src/">embedded-hal-async</a> for the desired MCU/LoRa chip combination.

## How?

- separate out modulation parameters and packet parameters as separate concerns to address the nuances in LoRa chip support and to allow flexible specification of various LoRaWAN and P2P send/receive channels, even in the same use case;
- allow the user to specify a LoRa chip kind (for example, Sx1261/2) and specific LoRa board type (for example, Stm32wlSx1262) and hide the control of that LoRa board behind the LoRa physical layer API;
- provide a minimal trait which must be implemented for each desired embedded framework/MCU type/LoRa chip type to allow this crate to interface to the LoRa chip within the embedded framework.

## Wheretofore?

- while the current examples use the Embassy embedded framework, nrf52840, stm32wl, and stm32l0 MCUs, and Sx127x/Sx126x chips, this crate provides a path forward for other embedded frameworks, MCU types, and LoRa chips in a Rust development environment;
- the links below refer to a fork of Embassy and/or the lora crate in the <a href="https://github.com/ceekdee">ceekdee GitHub repository</a>; however, the intent is to update Embassy to support this lora crate and to move this lora crate to an appropriate embedded framework-agnostic repository;
- the existing LoRa implementations in Embassy remain available after the update to support this lora crate.

## LoRa physical layer API

For users wishing to implement a LoRaWAN or P2P solution, the following implementation files provide the necessary context:

- <a href="https://github.com/ceekdee/lora/blob/main/src/lib.rs">the API itself</a>;
- <a href="https://github.com/ceekdee/lora/blob/main/src/mod_params.rs">pertinent ancillary information</a>.

Examples of API usage:

- <a href="https://github.com/ceekdee/embassy/blob/master/examples/stm32wl/src/bin/lora_p2p_send.rs">stm32wl P2P send and sleep</a>;
- <a href="https://github.com/ceekdee/embassy/blob/master/examples/stm32l0/src/bin/lora_p2p_receive.rs">stm32l0 P2P receive continuous</a>;
- <a href="https://github.com/ceekdee/embassy/blob/master/examples/nrf52840/src/bin/lora_p2p_receive_duty_cycle.rs">nrf52840 duty cycle receive</a>;
- <a href="https://github.com/ceekdee/embassy/blob/master/examples/nrf52840/src/bin/lora_cad.rs">nrf52840 channel activity detection</a>.

## Embedded framework/MCU support

For embedded framework developers wishing to add LoRa support as a feature for one or more MCU/LoRa chip combinations:

- <a href="https://github.com/ceekdee/lora/blob/main/src/mod_traits.rs">the InterfaceVariant trait</a>, which enables this lora crate to interface to a specific embedded framework/MCU/LoRa chip combination.

Example InterfaceVariant implementations:

- <a href="https://github.com/ceekdee/embassy/blob/master/embassy-lora/src/iv.rs">Embassy stm32wl/Sx1262, stm32l0/Sx1276, and nrf52840/Sx1262</a>

## LoRa chip support

For developers wishing to add support for new LoRa chips or enhance support for existing chips:

- <a href="https://github.com/ceekdee/lora/blob/main/src/mod_traits.rs">the RadioKind trait</a>, which must be implemented for each kind of LoRa chip for access through the lora crate API;
- <a href="https://github.com/ceekdee/lora/blob/main/src/interface.rs">the interface implementation</a>, which captures the three key read/write operations allowing control of the LoRa chip from this crate through either opcode or register operations.

Example RadioKind implementations and ancillary information:

- <a href="https://github.com/ceekdee/lora/tree/main/src/sx1261_2">the Sx1261/2 radio kind</a>;
- <a href="https://github.com/ceekdee/lora/tree/main/src/sx1276_7_8_9">the Sx1276/7/8/9 radio kind</a>.
