# lorawan-device

[![Build Status](https://travis-ci.org/ivajloip/rust-lorawan.svg?branch=master)](https://travis-ci.org/ivajloip/rust-lorawan)

This is an experimental LoRaWAN device stack. It can be tested by using the example in [the sx12xx-rs repository](https://github.com/lthiery/sx12xx-rs).
This is a very state machine driven implementation of a LoRaWAN stack designed for concurrency in non-threaded environments.
The state machine implementation was based off [an article by Ana Hobden](https://hoverbear.org/blog/rust-state-machine-pattern/).

There are two super-states that the Device can be in:
* **NoSession**: default state upon initialization
* **Session**: achieved after successful OTAA

State machine diagrams are provided in their respective files: `src/state_machines/no_session` and `src/state_machines/session`   

The following features are implemented:
* a radio abstraction layer by the following traits defined here: `radio::PhyRxTx + Timings`
* the `radio::PhyRxTx` trait enables a state machine design by the implementor
* a pass through for the LoRaWAN crypto abstraction provided by the lorawan-encoding, paving the way for secure elements and other hardware peripherals
* RX windows for data and join accepts are implemented by `Timeouts` which are passed by Response up to the user, minimizing borrowing or owning of such bindings by the library
* Timeouts can be adjusted by the radio abstraction layer thanks to the `Timing` trait
* the stack starts deriving a new session when the FCnt maxes out the 32-bit counter; new session may also be created by any time by the user, as long the stack is not mid-transmit
* MAC commands are minimally mocked, as a ADRReq is responded with an ADRResp, but not much is done with the actual payload

This is a work in progress and the notable limitations are:
* OTAA only, no ABP
* supports US915 only and subband 2 only
* no retries on Joins or Confirmed packets and the user is instead given **NoAck** and **NoJoinAccept** responses


