use super::super::session::Session;
use super::super::State as SuperState;
use super::super::*;
use super::{CommonState, Shared};
use lorawan_encoding::{
    self,
    creator::JoinRequestCreator,
    keys::AES128,
    parser::DevAddr,
    parser::{parse as lorawan_parse, *},
};

pub enum NoSession<R>
where
    R: radio::PhyRxTx + Timings,
{
    Idle(Idle<R>),
    SendingJoin(SendingJoin<R>),
    WaitingForRxWindow(WaitingForRxWindow<R>),
    WaitingForJoinResponse(WaitingForJoinResponse<R>),
}

macro_rules! into_state {
    ($($from:tt),*) => {
    $(
        impl<R> From<$from<R>> for Device<R>
        where
            R: radio::PhyRxTx + Timings,
        {
            fn from(state: $from<R>) -> Device<R> {
                Device { state: SuperState::NoSession(NoSession::$from(state)) }
            }
        }

        impl<R: radio::PhyRxTx + Timings> CommonState<R> for $from<R> {
            fn get_mut_shared(&mut self) -> &mut Shared<R> {
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

impl<R> From<NoSession<R>> for SuperState<R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(no_session: NoSession<R>) -> SuperState<R> {
        SuperState::NoSession(no_session)
    }
}

impl<R> NoSession<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn new(shared: Shared<R>) -> NoSession<R> {
        NoSession::Idle(Idle {
            shared,
            join_attempts: 0,
        })
    }

    pub fn get_mut_shared(&mut self) -> &mut Shared<R> {
        match self {
            NoSession::Idle(state) => state.get_mut_shared(),
            NoSession::SendingJoin(state) => state.get_mut_shared(),
            NoSession::WaitingForRxWindow(state) => state.get_mut_shared(),
            NoSession::WaitingForJoinResponse(state) => state.get_mut_shared(),
        }
    }

    pub fn handle_event(
        self,
        event: Event<R>,
    ) -> (Device<R>, Result<Response, super::super::Error<R>>) {
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

pub struct Idle<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    join_attempts: usize,
}

impl<'a, R> Idle<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event(
        mut self,
        event: Event<R>,
    ) -> (Device<R>, Result<Response, super::super::Error<R>>) {
        match event {
            // NewSession Request or a Timeout from previously failed Join attempt
            Event::NewSession | Event::Timeout => {
                let (devnonce, tx_config) = self.create_join_request();
                let radio_event: radio::Event<R> =
                    radio::Event::TxRequest(tx_config, &mut self.shared.buffer);

                // send the transmit request to the radio
                match self.shared.radio.handle_event(radio_event) {
                    Ok(response) => {
                        match response {
                            // intermediate state where we wait for Join to complete sending
                            // allows for asynchronous sending
                            radio::Response::Txing => (
                                self.into_sending_join(devnonce).into(),
                                Ok(Response::SendingJoinRequest),
                            ),
                            // directly jump to waiting for RxWindow
                            // allows for synchronous sending
                            radio::Response::TxDone(ms) => {
                                let time = join_rx_window_timeout(&self.shared.region, ms) as i32
                                    + self.shared.radio.get_rx_window_offset_ms();
                                (
                                    self.into_waiting_rxwindow(devnonce).into(),
                                    Ok(Response::TimeoutRequest(time as u32)),
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
            Event::SendData(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }

    fn create_join_request(&mut self) -> (DevNonce, radio::TxConfig) {
        let mut random = (self.shared.get_random)();
        // use lowest 16 bits for devnonce
        let devnonce_bytes = random as u16;

        self.shared.buffer.clear();
        let mut phy = JoinRequestCreator::new();
        let creds = &self.shared.credentials;

        let devnonce = [devnonce_bytes as u8, (devnonce_bytes >> 8) as u8];

        phy.set_app_eui(EUI64::new(creds.appeui()).unwrap())
            .set_dev_eui(EUI64::new(creds.deveui()).unwrap())
            .set_dev_nonce(&devnonce);
        let vec = phy.build(&creds.appkey()).unwrap();

        let devnonce_copy = DevNonce::new(devnonce).unwrap();

        self.shared.buffer.extend(vec);

        // we'll use the rest for frequency and subband selection
        random >>= 16;
        let frequency = self.shared.region.get_join_frequency(random as u8);

        let tx_config = radio::TxConfig {
            pw: 20,
            rf: radio::RfConfig {
                frequency,
                bandwidth: radio::Bandwidth::_125KHZ,
                spreading_factor: radio::SpreadingFactor::_10,
                coding_rate: radio::CodingRate::_4_5,
            },
        };
        (devnonce_copy, tx_config)
    }

    fn into_sending_join(self, devnonce: DevNonce) -> SendingJoin<R> {
        SendingJoin {
            shared: self.shared,
            join_attempts: self.join_attempts + 1,
            devnonce,
        }
    }

    fn into_waiting_rxwindow(self, devnonce: DevNonce) -> WaitingForRxWindow<R> {
        WaitingForRxWindow {
            shared: self.shared,
            join_attempts: self.join_attempts + 1,
            devnonce,
        }
    }
}

pub struct SendingJoin<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    join_attempts: usize,
    devnonce: DevNonce,
}

impl<R> SendingJoin<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event(
        mut self,
        event: Event<R>,
    ) -> (Device<R>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio

                let offset = self.shared.radio.get_rx_window_offset_ms();

                match self.shared.radio.handle_event(radio_event) {
                    Ok(response) => {
                        match response {
                            // expect a complete transmit
                            radio::Response::TxDone(ms) => {
                                let time =
                                    join_rx_window_timeout(&self.shared.region, ms) as i32 + offset;
                                (
                                    WaitingForRxWindow::from(self).into(),
                                    Ok(Response::TimeoutRequest(time as u32)),
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
            Event::NewSession => (
                self.into(),
                Err(Error::NewSessionWhileWaitingForJoinResponse.into()),
            ),
            Event::Timeout => panic!("TODO: implement timeouts"),
            Event::SendData(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }
}

impl<R> From<SendingJoin<R>> for WaitingForRxWindow<R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(val: SendingJoin<R>) -> WaitingForRxWindow<R> {
        WaitingForRxWindow {
            shared: val.shared,
            join_attempts: val.join_attempts,
            devnonce: val.devnonce,
        }
    }
}

pub struct WaitingForRxWindow<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    join_attempts: usize,
    devnonce: DevNonce,
}

impl<R> WaitingForRxWindow<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event(
        mut self,
        event: Event<R>,
    ) -> (Device<R>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for a Timeout
            Event::Timeout => {
                let rx_config = radio::RfConfig {
                    frequency: self.shared.region.get_join_accept_frequency1(),
                    bandwidth: radio::Bandwidth::_500KHZ,
                    spreading_factor: radio::SpreadingFactor::_10,
                    coding_rate: radio::CodingRate::_4_5,
                };
                // configure the radio for the RX
                match self
                    .shared
                    .radio
                    .handle_event(radio::Event::RxRequest(rx_config))
                {
                    // TODO: pass timeout
                    Ok(_) => (
                        WaitingForJoinResponse::from(self).into(),
                        Ok(Response::WaitingForJoinAccept),
                    ),
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            Event::RadioEvent(_) => (
                self.into(),
                Err(Error::RadioEventWhileWaitingForJoinWindow.into()),
            ),
            Event::NewSession => (
                self.into(),
                Err(Error::NewSessionWhileWaitingForJoinWindow.into()),
            ),
            Event::SendData(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }
}

impl<R> From<WaitingForRxWindow<R>> for WaitingForJoinResponse<R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(val: WaitingForRxWindow<R>) -> WaitingForJoinResponse<R> {
        WaitingForJoinResponse {
            shared: val.shared,
            join_attempts: val.join_attempts,
            devnonce: val.devnonce,
        }
    }
}

pub struct WaitingForJoinResponse<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    join_attempts: usize,
    devnonce: DevNonce,
}

impl<R> WaitingForJoinResponse<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event(
        mut self,
        event: Event<R>,
    ) -> (Device<R>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match self.shared.radio.handle_event(radio_event) {
                    Ok(response) => match response {
                        radio::Response::RxDone(_quality) => {
                            let packet =
                                lorawan_parse(self.shared.radio.get_received_packet()).unwrap();

                            if let PhyPayload::JoinAccept(join_accept) = packet {
                                if let JoinAcceptPayload::Encrypted(encrypted) = join_accept {
                                    let credentials = &self.shared.credentials;

                                    let decrypt = encrypted.decrypt(credentials.appkey());
                                    if decrypt.validate_mic(credentials.appkey()) {
                                        let session = SessionData::derive_new(
                                            &decrypt,
                                            self.devnonce,
                                            credentials,
                                        );
                                        return (
                                            Session::new(self.shared, session).into(),
                                            Ok(Response::NewSession),
                                        );
                                    }
                                }
                            }
                            (self.into(), Ok(Response::WaitingForJoinAccept))
                        }
                        _ => (self.into(), Ok(Response::WaitingForJoinAccept)),
                    },
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            Event::Timeout => panic!("TODO: implement Timeouts"),
            Event::NewSession => (
                self.into(),
                Err(Error::NewSessionWhileWaitingForJoinResponse.into()),
            ),
            Event::SendData(_) => (self.into(), Err(Error::SendDataWhileNoSession.into())),
        }
    }
}

impl<R> From<WaitingForJoinResponse<R>> for Idle<R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(val: WaitingForJoinResponse<R>) -> Idle<R> {
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

fn join_rx_window_timeout(region: &RegionalConfiguration, timestamp_ms: TimestampMs) -> u32 {
    region.get_join_accept_delay1() + timestamp_ms
}
