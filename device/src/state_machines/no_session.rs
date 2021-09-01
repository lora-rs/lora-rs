use super::super::session::Session;
use super::super::State as SuperState;
use super::super::*;
use super::{
    region::{Frame, Window},
    CommonState, Shared,
};
use lorawan_encoding::{
    self,
    creator::JoinRequestCreator,
    keys::AES128,
    parser::DevAddr,
    parser::{parse_with_factory as lorawan_parse, *},
};

pub enum NoSession<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    Idle(Idle<'a, R>),
    SendingJoin(SendingJoin<'a, R>),
    WaitingForRxWindow(WaitingForRxWindow<'a, R>),
    WaitingForJoinResponse(WaitingForJoinResponse<'a, R>),
}

enum JoinRxWindow {
    _1(u32),
    _2(u32),
}

macro_rules! into_state {
    ($($from:tt),*) => {
    $(
        impl<'a, R, C> From<$from<'a, R>> for Device<'a, R,C>
        where
            R: radio::PhyRxTx + Timings,
            C: CryptoFactory + Default
        {
            fn from(state: $from<'a, R>) -> Device<'a, R, C> {
                Device {
                    crypto: PhantomData::default(),
                    state: SuperState::NoSession(NoSession::$from(state))
                }
            }
        }

        impl<'a, R: radio::PhyRxTx + Timings> CommonState<'a, R> for $from<'a, R> {
            fn get_mut_shared(&mut self) -> &mut Shared<'a, R> {
                &mut self.shared
            }
        }
    )*};
}

into_state![
    Idle,
    SendingJoin,
    WaitingForRxWindow,
    WaitingForJoinResponse
];

impl<'a, R> From<NoSession<'a, R>> for SuperState<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(no_session: NoSession<'a, R>) -> SuperState<'a, R> {
        SuperState::NoSession(no_session)
    }
}

impl<'a, R> NoSession<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn new(shared: Shared<'a, R>) -> NoSession<'a, R> {
        NoSession::Idle(Idle {
            shared,
            join_attempts: 0,
        })
    }

    pub fn get_mut_shared(&mut self) -> &mut Shared<'a, R> {
        match self {
            NoSession::Idle(state) => state.get_mut_shared(),
            NoSession::SendingJoin(state) => state.get_mut_shared(),
            NoSession::WaitingForRxWindow(state) => state.get_mut_shared(),
            NoSession::WaitingForJoinResponse(state) => state.get_mut_shared(),
        }
    }

    pub fn handle_event<C: CryptoFactory + Default>(
        self,
        event: Event<R>,
    ) -> (Device<'a, R, C>, Result<Response, super::super::Error<R>>) {
        match self {
            NoSession::Idle(state) => state.handle_event(event),
            NoSession::SendingJoin(state) => state.handle_event(event),
            NoSession::WaitingForRxWindow(state) => state.handle_event(event),
            NoSession::WaitingForJoinResponse(state) => state.handle_event(event),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    RadioEventWhileIdle,
    SendDataWhileNoSession,
    RadioEventWhileWaitingForJoinWindow,
    NewSessionWhileWaitingForJoinWindow,
    SendDataWhileWaitingForJoinWindow,
    NewSessionWhileWaitingForJoinResponse,
}

impl<R> From<Error> for super::super::Error<R>
where
    R: radio::PhyRxTx,
{
    fn from(error: Error) -> super::super::Error<R> {
        super::super::Error::NoSession(error)
    }
}
type DevNonce = lorawan_encoding::parser::DevNonce<[u8; 2]>;

pub struct Idle<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<'a, R>,
    join_attempts: usize,
}

impl<'a, R> Idle<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<'a, R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            // NewSession Request or a Timeout from previously failed Join attempt
            Event::NewSessionRequest | Event::TimeoutFired => {
                let (devnonce, tx_config) = self.create_join_request::<C>();
                let radio_event: radio::Event<R> =
                    radio::Event::TxRequest(tx_config, &self.shared.tx_buffer.as_ref());

                // send the transmit request to the radio
                match self.shared.radio.handle_event(radio_event) {
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
                                    self.shared.region.get_rx_delay(&Frame::Join, &Window::_1) + ms;
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
            }
            Event::RadioEvent(_radio_event) => {
                (self.into(), Err(Error::RadioEventWhileIdle.into()))
            }
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }

    fn create_join_request<C: CryptoFactory + Default>(&mut self) -> (DevNonce, radio::TxConfig) {
        let mut random = (self.shared.get_random)();
        // use lowest 16 bits for devnonce
        let devnonce_bytes = random as u16;

        self.shared.tx_buffer.clear();

        let mut phy: JoinRequestCreator<[u8; 23], C> = JoinRequestCreator::default();
        let creds = &self.shared.credentials;

        let devnonce = [devnonce_bytes as u8, (devnonce_bytes >> 8) as u8];

        phy.set_app_eui(EUI64::new(creds.appeui()).unwrap())
            .set_dev_eui(EUI64::new(creds.deveui()).unwrap())
            .set_dev_nonce(&devnonce);
        let vec = phy.build(&creds.appkey()).unwrap();

        let devnonce_copy = DevNonce::new(devnonce).unwrap();

        self.shared.tx_buffer.extend_from_slice(vec).unwrap();

        // we'll use the rest for frequency and subband selection
        random >>= 16;
        (
            devnonce_copy,
            self.shared
                .region
                .create_tx_config(random as u8, self.shared.datarate, &Frame::Join),
        )
    }

    fn into_sending_join(self, devnonce: DevNonce) -> SendingJoin<'a, R> {
        SendingJoin {
            shared: self.shared,
            join_attempts: self.join_attempts + 1,
            devnonce,
        }
    }

    fn into_waiting_for_rxwindow(self, devnonce: DevNonce, time: u32) -> WaitingForRxWindow<'a, R> {
        WaitingForRxWindow {
            shared: self.shared,
            join_attempts: self.join_attempts + 1,
            join_rx_window: JoinRxWindow::_1(time),
            devnonce,
        }
    }
}

pub struct SendingJoin<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<'a, R>,
    join_attempts: usize,
    devnonce: DevNonce,
}

impl<'a, R> SendingJoin<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<'a, R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match self.shared.radio.handle_event(radio_event) {
                    Ok(response) => {
                        match response {
                            // expect a complete transmit
                            radio::Response::TxDone(ms) => {
                                let first_window =
                                    self.shared.region.get_rx_delay(&Frame::Join, &Window::_1)
                                        + ms
                                        + self.shared.radio.get_rx_window_offset_ms() as u32;
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
            Event::NewSessionRequest => (
                self.into(),
                Err(Error::NewSessionWhileWaitingForJoinResponse.into()),
            ),
            Event::TimeoutFired => panic!("TODO: implement timeouts"),
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }

    fn into_waiting_for_rxwindow(self, time: u32) -> WaitingForRxWindow<'a, R> {
        WaitingForRxWindow {
            shared: self.shared,
            join_attempts: self.join_attempts + 1,
            join_rx_window: JoinRxWindow::_1(time),
            devnonce: self.devnonce,
        }
    }
}

pub struct WaitingForRxWindow<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<'a, R>,
    join_attempts: usize,
    devnonce: DevNonce,
    join_rx_window: JoinRxWindow,
}

impl<'a, R> WaitingForRxWindow<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<'a, R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for a Timeout
            Event::TimeoutFired => {
                let window = match &self.join_rx_window {
                    JoinRxWindow::_1(_) => Window::_1,
                    JoinRxWindow::_2(_) => Window::_2,
                };
                let rx_config =
                    self.shared
                        .region
                        .get_rx_config(self.shared.datarate, &Frame::Join, &window);
                // configure the radio for the RX
                match self
                    .shared
                    .radio
                    .handle_event(radio::Event::RxRequest(rx_config))
                {
                    Ok(_) => {
                        let window_close: u32 = match self.join_rx_window {
                            // RxWindow1 one must timeout before RxWindow2
                            JoinRxWindow::_1(time) => {
                                let time_between_windows = self
                                    .shared
                                    .region
                                    .get_rx_delay(&Frame::Join, &Window::_2)
                                    - self.shared.region.get_rx_delay(&Frame::Join, &Window::_1);
                                if time_between_windows
                                    > self.shared.radio.get_rx_window_duration_ms()
                                {
                                    time + self.shared.radio.get_rx_window_duration_ms()
                                } else {
                                    time + time_between_windows
                                }
                            }
                            // RxWindow2 can last however long
                            JoinRxWindow::_2(time) => {
                                time + self.shared.radio.get_rx_window_duration_ms()
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
            Event::RadioEvent(_) => (
                self.into(),
                Err(Error::RadioEventWhileWaitingForJoinWindow.into()),
            ),
            Event::NewSessionRequest => (
                self.into(),
                Err(Error::NewSessionWhileWaitingForJoinWindow.into()),
            ),
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }
}

impl<'a, R> From<WaitingForRxWindow<'a, R>> for WaitingForJoinResponse<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(val: WaitingForRxWindow<'a, R>) -> WaitingForJoinResponse<'a, R> {
        WaitingForJoinResponse {
            join_rx_window: val.join_rx_window,
            shared: val.shared,
            join_attempts: val.join_attempts,
            devnonce: val.devnonce,
        }
    }
}

pub struct WaitingForJoinResponse<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<'a, R>,
    join_attempts: usize,
    devnonce: DevNonce,
    join_rx_window: JoinRxWindow,
}

impl<'a, R> WaitingForJoinResponse<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<'a, R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match self.shared.radio.handle_event(radio_event) {
                    Ok(response) => match response {
                        radio::Response::RxDone(_quality) => {
                            if let Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(
                                encrypted,
                            ))) =
                                lorawan_parse(self.shared.radio.get_received_packet(), C::default())
                            {
                                let credentials = &self.shared.credentials;
                                let decrypt = encrypted.decrypt(credentials.appkey());
                                self.shared.downlink = Some(super::Downlink::Join(
                                    self.shared.region.process_join_accept(&decrypt),
                                ));
                                if decrypt.validate_mic(credentials.appkey()) {
                                    let session = SessionData::derive_new(
                                        &decrypt,
                                        self.devnonce,
                                        credentials,
                                    );
                                    return (
                                        Session::new(self.shared, session).into(),
                                        Ok(Response::JoinSuccess),
                                    );
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
                if let Err(_e) = self.shared.radio.handle_event(radio::Event::CancelRx) {
                    panic!("Error cancelling Rx");
                }

                match self.join_rx_window {
                    JoinRxWindow::_1(t1) => {
                        let time_between_windows =
                            self.shared.region.get_rx_delay(&Frame::Join, &Window::_2)
                                - self.shared.region.get_rx_delay(&Frame::Join, &Window::_1);
                        let t2 = t1 + time_between_windows;
                        // TODO: jump to RxWindow2 if t2 == now
                        (
                            WaitingForRxWindow {
                                shared: self.shared,
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
                        Idle {
                            shared: self.shared,
                            join_attempts: self.join_attempts,
                        }
                        .into(),
                        Ok(Response::NoJoinAccept),
                    ),
                }
            }
            Event::NewSessionRequest => (
                self.into(),
                Err(Error::NewSessionWhileWaitingForJoinResponse.into()),
            ),
            Event::SendDataRequest(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }
}

impl<'a, R> From<WaitingForJoinResponse<'a, R>> for Idle<'a, R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(val: WaitingForJoinResponse<'a, R>) -> Idle<'a, R> {
        Idle {
            shared: val.shared,
            join_attempts: val.join_attempts,
        }
    }
}

pub struct SessionData {
    newskey: AES128,
    appskey: AES128,
    devaddr: DevAddr<[u8; 4]>,
    fcnt_up: u32,
    pub fcnt_down: u32,
}

impl SessionData {
    pub fn derive_new<T: core::convert::AsRef<[u8]>, F: lorawan_encoding::keys::CryptoFactory>(
        decrypt: &DecryptedJoinAcceptPayload<T, F>,
        devnonce: DevNonce,
        credentials: &Credentials,
    ) -> SessionData {
        SessionData {
            newskey: decrypt.derive_newskey(&devnonce, credentials.appkey()),
            appskey: decrypt.derive_appskey(&devnonce, credentials.appkey()),
            devaddr: DevAddr::new([
                decrypt.dev_addr().as_ref()[0],
                decrypt.dev_addr().as_ref()[1],
                decrypt.dev_addr().as_ref()[2],
                decrypt.dev_addr().as_ref()[3],
            ])
            .unwrap(),
            fcnt_up: 0,
            fcnt_down: 0,
        }
    }

    pub fn newskey(&self) -> &AES128 {
        &self.newskey
    }

    pub fn appskey(&self) -> &AES128 {
        &self.appskey
    }

    pub fn devaddr(&self) -> &DevAddr<[u8; 4]> {
        &self.devaddr
    }

    pub fn fcnt_up(&self) -> u32 {
        self.fcnt_up
    }

    pub fn fcnt_up_increment(&mut self) {
        self.fcnt_up += 1;
    }
}
