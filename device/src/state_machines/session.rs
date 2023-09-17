/*

This is the State Machine for a LoRaWan super-state "Session". The only way
to enter this state is for a device to be programmed in ABP mode (unimplemented)
or from a successul OTAA implemented in the NoSession module. The only way
to leave this state is to make a "Create Session" request which switches us
over to the "NoSession" super-state.

In this implementation, each state (eg: "Idle", "Txing") is a struct. When
an event is handled (eg: "SendData", "TxComplete"), a transition may or may
not occur. Regardless, a response is always given to the client, and those
are indicated here in paranthesis (ie: "(Sending)").

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
use super::super::no_session::NoSession;
use super::super::State as SuperState;
use super::super::*;
use super::region::{Frame, Window};

pub enum Session {
    Idle(Idle),
    SendingData(SendingData),
    WaitingForRxWindow(WaitingForRxWindow),
    WaitingForRx(WaitingForRx),
}

enum RxWindow {
    _1(u32),
    _2(u32),
}

trait SessionState {
    fn get_mac(&self) -> &Mac;
    fn get_session_keys(&self) -> &SessionKeys;
}

macro_rules! into_state {
    ($($from:tt),*) => {
    $(
        impl From<$from> for SuperState
        {
            fn from(state: $from) -> SuperState {
                SuperState::Session(Session::$from(state))
            }
        }

        impl SessionState for $from {
            fn get_session_keys(&self) -> &SessionKeys {
                &self.session
            }
            fn get_mac(&self) -> &Mac {
                &self.mac
            }
        }
    )*};
}

impl From<Session> for SuperState {
    fn from(session: Session) -> SuperState {
        SuperState::Session(session)
    }
}

into_state![Idle, SendingData, WaitingForRxWindow, WaitingForRx];

#[derive(Debug)]
pub enum Error {
    RadioEventWhileIdle,
    RadioEventWhileWaitingForRxWindow,
    NewSessionWhileWaitingForRxWindow,
    SendDataWhileWaitingForRxWindow,
    NewSessionWhileWaitingForRx,
    SendDataWhileWaitingForRx,
}

impl<R> From<Error> for super::Error<R> {
    fn from(error: Error) -> super::Error<R> {
        super::Error::Session(error)
    }
}

impl Session {
    pub fn new(session: SessionKeys, region: region::Configuration) -> Session {
        Session::Idle(Idle { mac: Mac::default(), session, region })
    }

    pub fn get_mac(&self) -> &Mac {
        match self {
            Session::Idle(state) => state.get_mac(),
            Session::SendingData(state) => state.get_mac(),
            Session::WaitingForRxWindow(state) => state.get_mac(),
            Session::WaitingForRx(state) => state.get_mac(),
        }
    }

    pub fn get_session_keys(&self) -> &SessionKeys {
        match self {
            Session::Idle(state) => state.get_session_keys(),
            Session::SendingData(state) => state.get_session_keys(),
            Session::WaitingForRxWindow(state) => state.get_session_keys(),
            Session::WaitingForRx(state) => state.get_session_keys(),
        }
    }

    pub fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        self,
        event: Event<R>,
        shared: &mut Shared<R, RNG, N>,
    ) -> (SuperState, Result<Response, super::Error<R::PhyError>>) {
        match self {
            Session::Idle(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            Session::SendingData(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            Session::WaitingForRxWindow(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            Session::WaitingForRx(state) => state.handle_event::<R, C, RNG, N>(event, shared),
        }
    }
}

impl Idle {
    pub fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        mut self,
        event: Event<R>,
        shared: &mut Shared<R, RNG, N>,
    ) -> (SuperState, Result<Response, super::Error<R::PhyError>>) {
        match event {
            Event::SendDataRequest(send_data) => {
                // encodes the packet and places it in send buffer
                let fcnt = self.mac.prepare_buffer::<C, N>(
                    &self.session,
                    &send_data,
                    &mut shared.tx_buffer,
                );
                let event: radio::Event<R> = radio::Event::TxRequest(
                    shared.region.create_tx_config(&mut shared.rng, shared.datarate, &Frame::Data),
                    shared.tx_buffer.as_ref(),
                );

                let confirmed = send_data.confirmed;

                // send the transmit request to the radio
                match shared.radio.handle_event(event) {
                    Ok(response) => {
                        match response {
                            // intermediate state where we wait for Join to complete sending
                            // allows for asynchronous sending
                            radio::Response::Txing => (
                                self.into_sending_data(confirmed).into(),
                                Ok(Response::UplinkSending(fcnt)),
                            ),
                            // directly jump to waiting for RxWindow
                            // allows for synchronous sending
                            radio::Response::TxDone(ms) => data_rxwindow1_timeout::<R, RNG, N>(
                                Session::Idle(self),
                                confirmed,
                                ms,
                                shared,
                            ),
                            _ => {
                                panic!("Idle: Unexpected radio response");
                            }
                        }
                    }
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            // tolerate unexpected timeout
            Event::TimeoutFired => (self.into(), Ok(Response::NoUpdate)),
            Event::NewSessionRequest => {
                let no_session = NoSession::new();
                no_session.handle_event::<R, C, RNG, N>(Event::NewSessionRequest, shared)
            }
            Event::RadioEvent(_radio_event) => {
                (self.into(), Err(Error::RadioEventWhileIdle.into()))
            }
        }
    }

    fn into_sending_data(self, confirmed: bool) -> SendingData {
        SendingData { mac: self.mac, confirmed, session: self.session, region: self.region }
    }

    fn into_waiting_for_rxwindow(self, confirmed: bool, time: u32) -> WaitingForRxWindow {
        WaitingForRxWindow {
            rx_window: RxWindow::_1(time),
            confirmed,
            mac: self.mac,
            session: self.session,
            region: self.region,
        }
    }
}

pub struct Idle {
    mac: Mac,
    session: SessionKeys,
    region: region::Configuration,
}

pub struct SendingData {
    mac: Mac,
    session: SessionKeys,
    region: region::Configuration,
    confirmed: bool,
}

impl SendingData {
    pub fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        self,
        event: Event<R>,
        shared: &mut Shared<R, RNG, N>,
    ) -> (SuperState, Result<Response, super::super::Error<R::PhyError>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match shared.radio.handle_event(radio_event) {
                    Ok(response) => {
                        match response {
                            // expect a complete transmit
                            radio::Response::TxDone(ms) => {
                                let confirmed = self.confirmed;
                                data_rxwindow1_timeout::<R, RNG, N>(
                                    Session::SendingData(self),
                                    confirmed,
                                    ms,
                                    shared,
                                )
                            }
                            // anything other than TxComplete is unexpected
                            _ => {
                                panic!("SendingData: Unexpected radio response");
                            }
                        }
                    }
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            // tolerate unexpected timeout
            Event::TimeoutFired => (self.into(), Ok(Response::NoUpdate)),
            // anything other than a RadioEvent is unexpected
            Event::NewSessionRequest | Event::SendDataRequest(_) => {
                panic!("Unexpected event while SendingJoin")
            }
        }
    }

    fn into_waiting_for_rxwindow(self, confirmed: bool, time: u32) -> WaitingForRxWindow {
        WaitingForRxWindow {
            rx_window: RxWindow::_1(time),
            confirmed,
            mac: self.mac,
            session: self.session,
            region: self.region,
        }
    }
}

pub struct WaitingForRxWindow {
    mac: Mac,
    session: SessionKeys,
    region: region::Configuration,
    confirmed: bool,
    rx_window: RxWindow,
}

impl WaitingForRxWindow {
    pub fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        self,
        event: Event<R>,
        shared: &mut Shared<R, RNG, N>,
    ) -> (SuperState, Result<Response, super::Error<R::PhyError>>) {
        match event {
            // we are waiting for a Timeout
            Event::TimeoutFired => {
                let window = match &self.rx_window {
                    RxWindow::_1(_) => Window::_1,
                    RxWindow::_2(_) => Window::_2,
                };
                let rx_config = shared.region.get_rx_config(shared.datarate, &Frame::Join, &window);

                // configure the radio for the RX
                match shared.radio.handle_event(radio::Event::RxRequest(rx_config)) {
                    Ok(_) => {
                        let window_close: u32 = match self.rx_window {
                            // RxWindow1 one must timeout before RxWindow2
                            RxWindow::_1(time) => {
                                let time_between_windows =
                                    shared.region.get_rx_delay(&Frame::Data, &Window::_2)
                                        - shared.region.get_rx_delay(&Frame::Data, &Window::_1);
                                if time_between_windows > shared.radio.get_rx_window_duration_ms() {
                                    time + shared.radio.get_rx_window_duration_ms()
                                } else {
                                    time + time_between_windows
                                }
                            }
                            // RxWindow2 can last however long
                            RxWindow::_2(time) => time + shared.radio.get_rx_window_duration_ms(),
                        };
                        (
                            WaitingForRx::from(self).into(),
                            Ok(Response::TimeoutRequest(window_close)),
                        )
                    }
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            Event::RadioEvent(_) => {
                (self.into(), Err(Error::RadioEventWhileWaitingForRxWindow.into()))
            }
            Event::NewSessionRequest => {
                (self.into(), Err(Error::NewSessionWhileWaitingForRxWindow.into()))
            }
            Event::SendDataRequest(_) => {
                (self.into(), Err(Error::SendDataWhileWaitingForRxWindow.into()))
            }
        }
    }
}

impl From<WaitingForRxWindow> for WaitingForRx {
    fn from(val: WaitingForRxWindow) -> WaitingForRx {
        WaitingForRx {
            confirmed: val.confirmed,
            rx_window: val.rx_window,
            mac: val.mac,
            session: val.session,
            region: val.region,
        }
    }
}

pub struct WaitingForRx {
    mac: Mac,
    session: SessionKeys,
    region: region::Configuration,
    confirmed: bool,
    rx_window: RxWindow,
}

impl WaitingForRx {
    pub fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        mut self,
        event: Event<R>,
        shared: &mut Shared<R, RNG, N>,
    ) -> (SuperState, Result<Response, super::Error<R::PhyError>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match shared.radio.handle_event(radio_event) {
                    Ok(response) => match response {
                        radio::Response::RxDone(_quality) => {
                            if let Some(response) = self.mac.handle_rx::<C>(
                                &self.session,
                                &mut self.region,
                                shared.radio.get_received_packet(),
                            ) {
                                return (self.into_idle().into(), Ok(response.into()));
                            }
                            (self.into(), Ok(Response::NoUpdate))
                        }
                        _ => (self.into(), Ok(Response::NoUpdate)),
                    },
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            Event::TimeoutFired => {
                // send the transmit request to the radio
                if let Err(_e) = shared.radio.handle_event(radio::Event::CancelRx) {
                    panic!("Error cancelling Rx");
                }

                match self.rx_window {
                    RxWindow::_1(t1) => {
                        let time_between_windows =
                            shared.region.get_rx_delay(&Frame::Data, &Window::_2)
                                - shared.region.get_rx_delay(&Frame::Data, &Window::_1);
                        let t2 = t1 + time_between_windows;
                        // TODO: jump to RxWindow2 if t2 == now
                        (
                            WaitingForRxWindow {
                                confirmed: self.confirmed,
                                rx_window: RxWindow::_2(t2),
                                mac: self.mac,
                                region: self.region,
                                session: self.session,
                            }
                            .into(),
                            Ok(Response::TimeoutRequest(t2)),
                        )
                    }
                    // Timeout during second RxWindow leads to giving up
                    RxWindow::_2(_) => {
                        let response = self.mac.rx2_elapsed();
                        (self.into_idle().into(), Ok(response.into()))
                    }
                }
            }
            Event::NewSessionRequest => {
                (self.into(), Err(Error::NewSessionWhileWaitingForRx.into()))
            }
            Event::SendDataRequest(_) => {
                (self.into(), Err(Error::SendDataWhileWaitingForRx.into()))
            }
        }
    }

    fn into_idle(self) -> Idle {
        Idle { mac: self.mac, session: self.session, region: self.region }
    }
}

fn data_rxwindow1_timeout<R: radio::PhyRxTx + Timings, RNG: RngCore, const N: usize>(
    state: Session,
    confirmed: bool,
    timestamp_ms: TimestampMs,
    shared: &Shared<R, RNG, N>,
) -> (SuperState, Result<Response, super::super::Error<R::PhyError>>) {
    let (new_state, first_window) = match state {
        Session::Idle(state) => {
            let first_window = (shared.region.get_rx_delay(&Frame::Data, &Window::_1) as i32
                + timestamp_ms as i32
                + shared.radio.get_rx_window_offset_ms()) as u32;
            (state.into_waiting_for_rxwindow(confirmed, first_window), first_window)
        }
        Session::SendingData(state) => {
            let first_window = (shared.region.get_rx_delay(&Frame::Data, &Window::_1) as i32
                + timestamp_ms as i32
                + shared.radio.get_rx_window_offset_ms()) as u32;
            (state.into_waiting_for_rxwindow(confirmed, first_window), first_window)
        }
        _ => panic!("Invalid state to transition to WaitingForRxWindow"),
    };

    (new_state.into(), Ok(Response::TimeoutRequest(first_window)))
}
