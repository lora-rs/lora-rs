/*

This state machine creates a non-blocking and no-async structure for coordinating radio events with
the mac state.

In this implementation, each state (eg: "Idle", "Txing") is a struct. When an event is handled
(eg: "SendData", "TxComplete"), a transition may or may not occur. Regardless, a response is always
given to the client, and those are indicated here in parenthesis (ie: "(Sending)"). If nothing is
indicated in this diagram, the response is "NoUpdate".

O
│
╔═══════════════════╗                                ╔════════════════════╗
║ Idle              ║                                ║ Txing              ║
║          SendData ║       if async       (Sending) ║                    ║
║          ─────────╫───────────────┬───────────────>║                    ║
║                   ║               │                ║         TxComplete ║
╚═══════════════════╝               │                ║          ──────────╫───┐
      ^                             │                ╚════════════════════╝   │
      │                             │                                         │
      │                             │                                         │
┌─────┘    ╔═══════════════════╗    │          ╔════════════════════╗         │
│          ║ WaitingForRx      ║    │          ║ WaitingForRxWindow ║         │
│          ║ ╔═════════════╗   ║    │else sync ║  ╔═════════════╗   ║         │
│          ║ ║ RxWindow1   ║   ║    └──────────╫─>║ RxWindow1   ║<──╫─────────┘
│(DataDown)║ ║    Rx       ║   ║   (TimeoutReq)║  ║             ║   ║(TimeoutReq)
├──────────╫─╫───────      ║   ║(TimeoutReq)   ║  ║    Timeout  ║   ║
│          ║ ║    Timeout  ║<──╫───────────────╫──╫──────────── ║   ║
│          ║ ║    ─────────╫───╫──┐            ║  ╚═════════════╝   ║
│          ║ ╚═════════════╝   ║  │            ║                    ║
│          ║ ╔═════════════╗   ║  │(TimeoutReq)║   ╔═════════════╗  ║
│(DataDown)║ ║ RxWindow2   ║   ║  └────────────╫─> ║ RxWindow2   ║  ║
├──────────╫─╫──┐ Rx       ║   ║               ║   ║             ║  ║
│          ║ ║  └───       ║   ║(TimeoutReq)   ║   ║    Timeout  ║  ║
│ if conf  ║ ║    Timeout  ║<──╫───────────────╫───╫──────────── ║  ║
│ (NoACK)  ║ ║   ┌──────── ║   ║               ║   ╚═════════════╝  ║
└──────────╫─╫───┘         ║   ║               ║                    ║
else(Ready)║ ╚═════════════╝   ║               ║                    ║
           ╚═══════════════════╝               ╚════════════════════╝
 */
use super::super::*;
use super::{
    mac::{Frame, Mac, Window},
    radio, Event, RadioBuffer, Response, Timings,
};

#[derive(Copy, Clone)]
pub enum State {
    Idle(Idle),
    SendingData(SendingData),
    WaitingForRxWindow(WaitingForRxWindow),
    WaitingForRx(WaitingForRx),
}

macro_rules! into_state {
    ($($from:tt),*) => {
    $(
        impl From<$from> for State
        {
            fn from(s: $from) -> State {
                State::$from(s)
            }
        }
    )*};
}

into_state!(Idle, SendingData, WaitingForRxWindow, WaitingForRx);

impl Default for State {
    fn default() -> Self {
        State::Idle(Idle)
    }
}

impl From<Rx> for Window {
    fn from(val: Rx) -> Window {
        match val {
            Rx::_1(_) => Window::_1,
            Rx::_2(_) => Window::_2,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    RadioEventWhileIdle,
    RadioEventWhileWaitingForRxWindow,
    NewSessionWhileWaitingForRxWindow,
    SendDataWhileWaitingForRxWindow,
    TxRequestDuringTx,
    NewSessionWhileWaitingForRx,
    SendDataWhileWaitingForRx,
    BufferTooSmall,
    UnexpectedRadioResponse,
}

impl<R: radio::PhyRxTx> From<Error> for super::Error<R> {
    fn from(error: Error) -> super::Error<R> {
        super::Error::State(error)
    }
}

impl State {
    pub(crate) fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
        const D: usize,
    >(
        self,
        mac: &mut Mac,
        radio: &mut R,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
        event: Event<R>,
    ) -> (Self, Result<Response, super::Error<R>>) {
        match self {
            State::Idle(s) => s.handle_event::<R, C, RNG, N>(mac, radio, rng, buf, event),
            State::SendingData(s) => s.handle_event::<R, N>(mac, radio, event),
            State::WaitingForRxWindow(s) => s.handle_event::<R, N>(mac, radio, event),
            State::WaitingForRx(s) => s.handle_event::<R, C, N, D>(mac, radio, buf, event, dl),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Idle;

impl Idle {
    pub(crate) fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        self,
        mac: &mut Mac,
        radio: &mut R,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
        event: Event<R>,
    ) -> (State, Result<Response, super::Error<R>>) {
        enum IntermediateResponse<R: radio::PhyRxTx> {
            RadioTx((Frame, radio::TxConfig, u32)),
            EarlyReturn(Result<Response, super::Error<R>>),
        }

        let response = match event {
            // tolerate unexpected timeout
            Event::Join(creds) => {
                let (tx_config, dev_nonce) = mac.join_otaa::<C, RNG, N>(rng, creds, buf);
                IntermediateResponse::RadioTx((Frame::Join, tx_config, dev_nonce as u32))
            }
            Event::TimeoutFired => IntermediateResponse::EarlyReturn(Ok(Response::NoUpdate)),
            Event::RadioEvent(_radio_event) => {
                IntermediateResponse::EarlyReturn(Err(Error::RadioEventWhileIdle.into()))
            }
            Event::SendDataRequest(send_data) => {
                let tx_config = mac.send::<C, RNG, N>(rng, buf, &send_data);
                match tx_config {
                    Err(e) => IntermediateResponse::EarlyReturn(Err(e.into())),
                    Ok((tx_config, fcnt_up)) => {
                        IntermediateResponse::RadioTx((Frame::Data, tx_config, fcnt_up))
                    }
                }
            }
        };
        match response {
            IntermediateResponse::EarlyReturn(response) => (State::Idle(self), response),
            IntermediateResponse::RadioTx((frame, tx_config, fcnt_up)) => {
                let event: radio::Event<R> =
                    radio::Event::TxRequest(tx_config, buf.as_ref_for_read());
                match radio.handle_event(event) {
                    Ok(response) => {
                        match response {
                            // intermediate state where we wait for Join to complete sending
                            // allows for asynchronous sending
                            radio::Response::Txing => (
                                State::SendingData(SendingData { frame }),
                                Ok(Response::UplinkSending(fcnt_up)),
                            ),
                            // directly jump to waiting for RxWindow
                            // allows for synchronous sending
                            radio::Response::TxDone(ms) => {
                                data_rxwindow1_timeout::<R, N>(frame, mac, radio, ms)
                            }
                            _ => (State::Idle(self), Err(Error::UnexpectedRadioResponse.into())),
                        }
                    }
                    Err(e) => (State::Idle(self), Err(super::Error::Radio(e))),
                }
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct SendingData {
    frame: Frame,
}

impl SendingData {
    pub(crate) fn handle_event<R: radio::PhyRxTx + Timings, const N: usize>(
        self,
        mac: &mut Mac,
        radio: &mut R,
        event: Event<R>,
    ) -> (State, Result<Response, super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match radio.handle_event(radio_event) {
                    Ok(response) => {
                        match response {
                            // expect a complete transmit
                            radio::Response::TxDone(ms) => {
                                data_rxwindow1_timeout::<R, N>(self.frame, mac, radio, ms)
                            }
                            // anything other than TxComplete is unexpected
                            _ => {
                                panic!("SendingData: Unexpected radio response");
                            }
                        }
                    }
                    Err(e) => (State::SendingData(self), Err(super::Error::Radio(e))),
                }
            }
            // tolerate unexpected timeout
            Event::TimeoutFired => (State::SendingData(self), Ok(Response::NoUpdate)),
            // anything other than a RadioEvent is unexpected
            Event::Join(_) | Event::SendDataRequest(_) => {
                (self.into(), Err(Error::TxRequestDuringTx.into()))
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct WaitingForRxWindow {
    frame: Frame,
    window: Rx,
}

impl WaitingForRxWindow {
    pub(crate) fn handle_event<R: radio::PhyRxTx + Timings, const N: usize>(
        self,
        mac: &mut Mac,
        radio: &mut R,
        event: Event<R>,
    ) -> (State, Result<Response, super::Error<R>>) {
        match event {
            // we are waiting for a Timeout
            Event::TimeoutFired => {
                let (rx_config, window_start) =
                    mac.get_rx_parameters_legacy(&self.frame, &self.window.into());
                // configure the radio for the RX
                match radio.handle_event(radio::Event::RxRequest(rx_config)) {
                    Ok(_) => {
                        let window_close: u32 = match self.window {
                            // RxWindow1 one must timeout before RxWindow2
                            Rx::_1(time) => {
                                let time_between_windows =
                                    mac.get_rx_delay(&self.frame, &Window::_2) - window_start;
                                if time_between_windows > radio.get_rx_window_duration_ms() {
                                    time + radio.get_rx_window_duration_ms()
                                } else {
                                    time + time_between_windows
                                }
                            }
                            // RxWindow2 can last however long
                            Rx::_2(time) => time + radio.get_rx_window_duration_ms(),
                        };
                        (
                            State::WaitingForRx(self.into()),
                            Ok(Response::TimeoutRequest(window_close)),
                        )
                    }
                    Err(e) => (State::WaitingForRxWindow(self), Err(super::Error::Radio(e))),
                }
            }
            Event::RadioEvent(_) => (
                State::WaitingForRxWindow(self),
                Err(Error::RadioEventWhileWaitingForRxWindow.into()),
            ),
            Event::Join(_) => (
                State::WaitingForRxWindow(self),
                Err(Error::NewSessionWhileWaitingForRxWindow.into()),
            ),
            Event::SendDataRequest(_) => (
                State::WaitingForRxWindow(self),
                Err(Error::SendDataWhileWaitingForRxWindow.into()),
            ),
        }
    }
}

impl From<WaitingForRxWindow> for WaitingForRx {
    fn from(val: WaitingForRxWindow) -> WaitingForRx {
        WaitingForRx { frame: val.frame, window: val.window }
    }
}

#[derive(Copy, Clone)]
pub struct WaitingForRx {
    frame: Frame,
    window: Rx,
}

impl WaitingForRx {
    pub(crate) fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        const N: usize,
        const D: usize,
    >(
        self,
        mac: &mut Mac,
        radio: &mut R,
        buf: &mut RadioBuffer<N>,
        event: Event<R>,
        dl: &mut Vec<Downlink, D>,
    ) -> (State, Result<Response, super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match radio.handle_event(radio_event) {
                    Ok(response) => match response {
                        radio::Response::RxDone(_quality) => {
                            // copy from radio buffer to mac buffer
                            buf.clear();
                            if let Err(()) =
                                buf.extend_from_slice(radio.get_received_packet().as_ref())
                            {
                                return (
                                    State::WaitingForRx(self),
                                    Err(Error::BufferTooSmall.into()),
                                );
                            }
                            match mac.handle_rx::<C, N, D>(buf, dl) {
                                // NoUpdate can occur when a stray radio packet is received. Maintain state
                                mac::Response::NoUpdate => {
                                    (State::WaitingForRx(self), Ok(Response::NoUpdate))
                                }
                                // Any other type of update indicates we are done receiving. Change to Idle
                                r => (State::Idle(Idle), Ok(r.into())),
                            }
                        }
                        _ => (State::WaitingForRx(self), Ok(Response::NoUpdate)),
                    },
                    Err(e) => (State::WaitingForRx(self), Err(super::Error::Radio(e))),
                }
            }
            Event::TimeoutFired => {
                if let Err(e) = radio.handle_event(radio::Event::CancelRx) {
                    return (State::WaitingForRx(self), Err(super::Error::Radio(e)));
                }

                match self.window {
                    Rx::_1(t1) => {
                        let time_between_windows = mac.get_rx_delay(&self.frame, &Window::_2)
                            - mac.get_rx_delay(&self.frame, &Window::_1);
                        let t2 = t1 + time_between_windows;
                        // TODO: jump to RxWindow2 if t2 == now
                        (
                            State::WaitingForRxWindow(WaitingForRxWindow {
                                frame: self.frame,
                                window: Rx::_2(t2),
                            }),
                            Ok(Response::TimeoutRequest(t2)),
                        )
                    }
                    // Timeout during second RxWindow leads to giving up
                    Rx::_2(_) => {
                        let response = mac.rx2_complete();
                        (State::Idle(Idle), Ok(response.into()))
                    }
                }
            }
            Event::Join(_) => {
                (State::WaitingForRx(self), Err(Error::NewSessionWhileWaitingForRx.into()))
            }
            Event::SendDataRequest(_) => {
                (State::WaitingForRx(self), Err(Error::SendDataWhileWaitingForRx.into()))
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum Rx {
    _1(u32),
    _2(u32),
}

fn data_rxwindow1_timeout<R: radio::PhyRxTx + Timings, const N: usize>(
    frame: Frame,
    mac: &mut Mac,
    radio: &mut R,
    timestamp_ms: u32,
) -> (State, Result<Response, super::Error<R>>) {
    let delay = mac.get_rx_delay(&frame, &Window::_1);
    let t1 = (delay as i32 + timestamp_ms as i32 + radio.get_rx_window_offset_ms()) as u32;
    (
        State::WaitingForRxWindow(WaitingForRxWindow { frame, window: Rx::_1(t1) }),
        Ok(Response::TimeoutRequest(t1)),
    )
}
