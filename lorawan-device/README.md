# lorawan-device

[![Gitter chat](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/rust-lorawan/lorawan)

This is an experimental LoRaWAN device stack. It can be tested by using the
example in [the sx12xx-rs repository](https://github.com/lthiery/sx12xx-rs). You
may also consider the example from
[the Drogue Device framework](https://github.com/drogue-iot/drogue-device/).

The device stack supports two modes, both designed for concurrency in non-threaded environments:

* A state machine implementation based off [an article by Ana Hobden](https://hoverbear.org/blog/rust-state-machine-pattern/).
* An async-await implementation that can be used with async radio interfaces.

## State machine implementation

There are two super-states that the Device can be in:

- **NoSession**: default state upon initialization
- **Session**: achieved after successful OTAA

A state machine diagram is provided in `src/state_machines/session`

The following LoRaWAN features are implemented:

- Class A device behavior
- Over-the-Air Activation (OTAA) and Activation by Personalization (ABP)
- Regional support for US915, and EU868
- Supports CFList in JoinAccept for EU868
- the stack starts deriving a new session when the FCnt maxes out the 32-bit
  counter; new session may also be created by any time by the user, as long the
  stack is not mid-transmit
- MAC commands are minimally mocked, as a ADRReq is responded with an ADRResp,
  but not much is done with the actual payload

The following design features are implemented:

- a radio abstraction layer by the following traits defined here:
  `radio::PhyRxTx + Timings`
- the `radio::PhyRxTx` trait enables a state machine design by the implementor
- a pass through for the LoRaWAN crypto abstraction provided by the
  lorawan crate, paving the way for secure elements and other hardware
  peripherals
- RX windows for data and join accepts are implemented by `Timeouts` which are
  passed by Response up to the user, minimizing borrowing or owning of such
  bindings by the library
- Timeouts can be adjusted by the radio abstraction layer thanks to the `Timing`
  trait

This is a work in progress and the notable limitations are:

- Class A behavior only, not B or C
- no retries on Joins or Confirmed packets and the user is instead given
  **NoAck** and **NoJoinAccept** responses

## Async implementation

The async implementation uses the async-await capabilities of Rust to drive the state machine. It
differs from the state machine implementation in the following ways:

* Join, send and send_recv are all async methods that can be awaited until the state transition is
  complete.
* When a session is expired, a SessionExpired error will be returned when attempting to send data. A
  new call to join must be made to establish a new session.
* The radio implementation is fully async
* A trait for an asynchronous timer is defined and must be implemented to use the async stack
* Uses the RngCore trait for random number generation

In terms of features, the async stack supports the same set of features as the state machine driven
implementation.
