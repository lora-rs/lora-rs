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
    mac::Mac,
    region::{Frame, Window},
    RadioBuffer,
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
    NewSessionWhileWaitingForRx,
    SendDataWhileWaitingForRx,
    BufferTooSmall,
    UnexpectedRadioResponse,
}

impl State {
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
        dl: &mut Option<Downlink>,
        event: Event<R>,
    ) -> (Self, Result<Response, super::Error<R::PhyError>>) {
        match self {
            State::Idle(s) => s.handle_event::<R, C, RNG, N>(mac, radio, rng, buf, event),
            State::SendingData(s) => s.handle_event::<R, N>(mac, radio, event),
            State::WaitingForRxWindow(s) => s.handle_event::<R, N>(mac, radio, event),
            State::WaitingForRx(s) => s.handle_event::<R, C, N>(mac, radio, buf, event, dl),
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
    ) -> (State, Result<Response, super::Error<R::PhyError>>) {
        enum IntermediateResponse<R> {
            RadioTx((Frame, radio::TxConfig)),
            EarlyReturn(Result<Response, super::Error<R>>),
        }

        let response = match event {
            // tolerate unexpected timeout
            Event::Join(creds) => IntermediateResponse::RadioTx((
                Frame::Join,
                mac.join_otaa::<C, RNG, N>(rng, creds, buf),
            )),
            Event::TimeoutFired => IntermediateResponse::EarlyReturn(Ok(Response::NoUpdate)),
            Event::RadioEvent(_radio_event) => {
                IntermediateResponse::EarlyReturn(Err(Error::RadioEventWhileIdle.into()))
            }
            Event::SendDataRequest(send_data) => {
                let tx_config = mac.send::<C, RNG, N>(rng, buf, &send_data);
                match tx_config {
                    Err(e) => IntermediateResponse::EarlyReturn(Err(e.into())),
                    Ok(tx_config) => IntermediateResponse::RadioTx((Frame::Data, tx_config)),
                }
            }
        };
        match response {
            IntermediateResponse::EarlyReturn(response) => (State::Idle(self), response),
            IntermediateResponse::RadioTx((frame, tx_config)) => {
                let event: radio::Event<R> =
                    radio::Event::TxRequest(tx_config, buf.as_ref_for_read());
                match radio.handle_event(event) {
                    Ok(response) => {
                        match response {
                            // intermediate state where we wait for Join to complete sending
                            // allows for asynchronous sending
                            radio::Response::Txing => (
                                State::SendingData(SendingData { frame }),
                                // TODO: get fcnt from mac
                                Ok(Response::UplinkSending(0)),
                            ),
                            // directly jump to waiting for RxWindow
                            // allows for synchronous sending
                            radio::Response::TxDone(ms) => {
                                data_rxwindow1_timeout::<R, N>(frame, mac, radio, ms)
                            }
                            _ => (State::Idle(self), Err(Error::UnexpectedRadioResponse.into())),
                        }
                    }
                    Err(e) => (State::Idle(self), Err(e.into())),
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
    ) -> (State, Result<Response, super::Error<R::PhyError>>) {
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
                    Err(e) => (State::SendingData(self), Err(e.into())),
                }
            }
            // tolerate unexpected timeout
            Event::TimeoutFired => (State::SendingData(self), Ok(Response::NoUpdate)),
            // anything other than a RadioEvent is unexpected
            Event::Join(_) | Event::SendDataRequest(_) => {
                panic!("Unexpected event while SendingJoin")
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
    ) -> (State, Result<Response, super::Error<R::PhyError>>) {
        match event {
            // we are waiting for a Timeout
            Event::TimeoutFired => {
                // TODO: data frame vs join frame?
                let (rx_config, window_start) =
                    mac.get_rx_parameters(&self.frame, &self.window.into());
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
                    Err(e) => (State::WaitingForRxWindow(self), Err(e.into())),
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
    >(
        self,
        mac: &mut Mac,
        radio: &mut R,
        buf: &mut RadioBuffer<N>,
        event: Event<R>,
        dl: &mut Option<Downlink>,
    ) -> (State, Result<Response, super::Error<R::PhyError>>) {
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
                            match mac.handle_rx::<C, N>(buf, dl) {
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
                    Err(e) => (State::WaitingForRx(self), Err(e.into())),
                }
            }
            Event::TimeoutFired => {
                if let Err(e) = radio.handle_event(radio::Event::CancelRx) {
                    return (State::WaitingForRx(self), Err(e.into()));
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
) -> (State, Result<Response, super::Error<R::PhyError>>) {
    let delay = mac.get_rx_delay(&frame, &Window::_1);
    let t1 = (delay as i32 + timestamp_ms as i32 + radio.get_rx_window_offset_ms()) as u32;
    (
        State::WaitingForRxWindow(WaitingForRxWindow { frame, window: Rx::_1(t1) }),
        Ok(Response::TimeoutRequest(t1)),
    )
}
