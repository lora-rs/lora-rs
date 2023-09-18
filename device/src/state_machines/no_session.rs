use super::super::session::Session;
use super::super::State as SuperState;
use super::super::*;
use super::{
    region::{Frame, Window},
    RngCore, Shared,
};
use lorawan::{
    self,
    parser::parse_with_factory as lorawan_parse,
    parser::{JoinAcceptPayload, PhyPayload},
};

pub enum NoSession {
    Idle(Idle),
    SendingJoin(SendingJoin),
    WaitingForRxWindow(WaitingForRxWindow),
    WaitingForJoinResponse(WaitingForJoinResponse),
}

enum JoinRxWindow {
    _1(u32),
    _2(u32),
}

macro_rules! into_state {
    ($($from:tt),*) => {
    $(
        impl From<$from> for SuperState
        {
            fn from(state: $from) -> SuperState {
                SuperState::NoSession(NoSession::$from(state))
            }
        }
    )*};
}

into_state![Idle, SendingJoin, WaitingForRxWindow, WaitingForJoinResponse];

impl From<NoSession> for SuperState {
    fn from(no_session: NoSession) -> SuperState {
        SuperState::NoSession(no_session)
    }
}

impl Default for NoSession {
    fn default() -> Self {
        NoSession::Idle(Idle { join_attempts: 0 })
    }
}

impl NoSession {
    pub fn new() -> NoSession {
        Self::default()
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
            NoSession::Idle(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            NoSession::SendingJoin(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            NoSession::WaitingForRxWindow(state) => {
                state.handle_event::<R, C, RNG, N>(event, shared)
            }
            NoSession::WaitingForJoinResponse(state) => {
                state.handle_event::<R, C, RNG, N>(event, shared)
            }
        }
    }
}

#[derive(Debug)]
pub enum Error {
    DeviceDoesNotHaveOtaaCredentials,
    RadioEventWhileIdle,
    SendDataWhileNoSession,
    RadioEventWhileWaitingForJoinWindow,
    NewSessionWhileWaitingForJoinWindow,
    SendDataWhileWaitingForJoinWindow,
    NewSessionWhileWaitingForJoinResponse,
}

impl<R> From<Error> for super::Error<R> {
    fn from(error: Error) -> super::Error<R> {
        super::Error::NoSession(error)
    }
}

pub struct Idle {
    join_attempts: usize,
}

impl Idle {
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
            // NewSession Request or a Timeout from previously failed Join attempt
            Event::NewSessionRequest | Event::TimeoutFired => {
                if let Some(credentials) = &shared.credentials {
                    let (devnonce, tx_config) = credentials.create_join_request::<C, RNG, N>(
                        &mut shared.region,
                        &mut shared.rng,
                        shared.datarate,
                        &mut shared.tx_buffer,
                    );
                    let radio_event: radio::Event<R> =
                        radio::Event::TxRequest(tx_config, shared.tx_buffer.as_ref());

                    // send the transmit request to the radio
                    match shared.radio.handle_event(radio_event) {
                        Ok(response) => {
                            match response {
                                // intermediate state where we wait for Join to complete sending
                                // allows for asynchronous sending
                                radio::Response::Txing => (
                                    self.into_sending_join(devnonce).into(),
                                    Ok(Response::JoinRequestSending),
                                ),
                                // directly jump to waiting for RxWindow
                                // allows for synchronous sending
                                radio::Response::TxDone(ms) => {
                                    let first_window =
                                        (shared.region.get_rx_delay(&Frame::Join, &Window::_1)
                                            as i32
                                            + ms as i32
                                            + shared.radio.get_rx_window_offset_ms())
                                            as u32;
                                    (
                                        self.into_waiting_for_rxwindow(devnonce, first_window)
                                            .into(),
                                        Ok(Response::TimeoutRequest(first_window)),
                                    )
                                }
                                _ => {
                                    panic!("NoSession::Idle:: Unexpected radio response");
                                }
                            }
                        }
                        Err(e) => (self.into(), Err(e.into())),
                    }
                } else {
                    (self.into(), Err(Error::DeviceDoesNotHaveOtaaCredentials.into()))
                }
            }
            Event::RadioEvent(_radio_event) => {
                (self.into(), Err(Error::RadioEventWhileIdle.into()))
            }
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }

    fn into_sending_join(self, devnonce: DevNonce) -> SendingJoin {
        SendingJoin { join_attempts: self.join_attempts + 1, devnonce }
    }

    fn into_waiting_for_rxwindow(self, devnonce: DevNonce, time: u32) -> WaitingForRxWindow {
        WaitingForRxWindow {
            join_attempts: self.join_attempts + 1,
            join_rx_window: JoinRxWindow::_1(time),
            devnonce,
        }
    }
}

pub struct SendingJoin {
    join_attempts: usize,
    devnonce: DevNonce,
}

impl SendingJoin {
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
                                let first_window =
                                    (shared.region.get_rx_delay(&Frame::Join, &Window::_1) as i32
                                        + ms as i32
                                        + shared.radio.get_rx_window_offset_ms())
                                        as u32;
                                (
                                    self.into_waiting_for_rxwindow(first_window).into(),
                                    Ok(Response::TimeoutRequest(first_window)),
                                )
                            }
                            // anything other than TxComplete | Idle is unexpected
                            _ => {
                                panic!("SendingJoin: Unexpected radio response");
                            }
                        }
                    }
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            // anything other than a RadioEvent is unexpected
            Event::NewSessionRequest => {
                (self.into(), Err(Error::NewSessionWhileWaitingForJoinResponse.into()))
            }
            Event::TimeoutFired => panic!("TODO: implement timeouts"),
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }

    fn into_waiting_for_rxwindow(self, time: u32) -> WaitingForRxWindow {
        WaitingForRxWindow {
            join_attempts: self.join_attempts + 1,
            join_rx_window: JoinRxWindow::_1(time),
            devnonce: self.devnonce,
        }
    }
}

pub struct WaitingForRxWindow {
    join_attempts: usize,
    devnonce: DevNonce,
    join_rx_window: JoinRxWindow,
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
                let window = match &self.join_rx_window {
                    JoinRxWindow::_1(_) => Window::_1,
                    JoinRxWindow::_2(_) => Window::_2,
                };
                let rx_config = shared.region.get_rx_config(shared.datarate, &Frame::Join, &window);
                // configure the radio for the RX
                match shared.radio.handle_event(radio::Event::RxRequest(rx_config)) {
                    Ok(_) => {
                        let window_close: u32 = match self.join_rx_window {
                            // RxWindow1 one must timeout before RxWindow2
                            JoinRxWindow::_1(time) => {
                                let time_between_windows =
                                    shared.region.get_rx_delay(&Frame::Join, &Window::_2)
                                        - shared.region.get_rx_delay(&Frame::Join, &Window::_1);
                                if time_between_windows > shared.radio.get_rx_window_duration_ms() {
                                    time + shared.radio.get_rx_window_duration_ms()
                                } else {
                                    time + time_between_windows
                                }
                            }
                            // RxWindow2 can last however long
                            JoinRxWindow::_2(time) => {
                                time + shared.radio.get_rx_window_duration_ms()
                            }
                        };
                        (
                            WaitingForJoinResponse::from(self).into(),
                            Ok(Response::TimeoutRequest(window_close)),
                        )
                    }
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            Event::RadioEvent(_) => {
                (self.into(), Err(Error::RadioEventWhileWaitingForJoinWindow.into()))
            }
            Event::NewSessionRequest => {
                (self.into(), Err(Error::NewSessionWhileWaitingForJoinWindow.into()))
            }
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }
}

impl From<WaitingForRxWindow> for WaitingForJoinResponse {
    fn from(val: WaitingForRxWindow) -> WaitingForJoinResponse {
        WaitingForJoinResponse {
            join_rx_window: val.join_rx_window,
            join_attempts: val.join_attempts,
            devnonce: val.devnonce,
        }
    }
}

pub struct WaitingForJoinResponse {
    join_attempts: usize,
    devnonce: DevNonce,
    join_rx_window: JoinRxWindow,
}

impl WaitingForJoinResponse {
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
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match shared.radio.handle_event(radio_event) {
                    Ok(response) => match response {
                        radio::Response::RxDone(_quality) => {
                            if let Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(
                                encrypted,
                            ))) = lorawan_parse(shared.radio.get_received_packet(), C::default())
                            {
                                match &shared.credentials {
                                    Some(credentials) => {
                                        let decrypt = encrypted.decrypt(&credentials.appkey().0);
                                        shared.region.process_join_accept(&decrypt);
                                        shared.downlink = Some(super::Downlink::Join);
                                        if decrypt.validate_mic(&credentials.appkey().0) {
                                            let session = SessionKeys::derive_new(
                                                &decrypt,
                                                self.devnonce,
                                                credentials,
                                            );
                                            return (
                                                Session::new(session, shared.region.clone()).into(),
                                                Ok(Response::JoinSuccess),
                                            );
                                        }
                                    }
                                    None => {
                                        return (
                                            self.into(),
                                            Err(Error::DeviceDoesNotHaveOtaaCredentials.into()),
                                        );
                                    }
                                }
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

                match self.join_rx_window {
                    JoinRxWindow::_1(t1) => {
                        let time_between_windows =
                            shared.region.get_rx_delay(&Frame::Join, &Window::_2)
                                - shared.region.get_rx_delay(&Frame::Join, &Window::_1);
                        let t2 = t1 + time_between_windows;
                        // TODO: jump to RxWindow2 if t2 == now
                        (
                            WaitingForRxWindow {
                                devnonce: self.devnonce,
                                join_attempts: self.join_attempts,
                                join_rx_window: JoinRxWindow::_2(t2),
                            }
                            .into(),
                            Ok(Response::TimeoutRequest(t2)),
                        )
                    }
                    // Timeout during second RxWindow leads to giving up
                    JoinRxWindow::_2(_) => (
                        Idle { join_attempts: self.join_attempts }.into(),
                        Ok(Response::NoJoinAccept),
                    ),
                }
            }
            Event::NewSessionRequest => {
                (self.into(), Err(Error::NewSessionWhileWaitingForJoinResponse.into()))
            }
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }
}

impl From<WaitingForJoinResponse> for Idle {
    fn from(val: WaitingForJoinResponse) -> Idle {
        Idle { join_attempts: val.join_attempts }
    }
}
