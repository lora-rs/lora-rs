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

use super::super::no_session::{NoSession, SessionData};
use super::super::State as SuperState;
use super::super::*;
use super::{
    region::{Frame, Window},
    CommonState,
};
use as_slice::AsSlice;
use generic_array::{typenum::U256, GenericArray};
use lorawan_encoding::{
    self,
    creator::DataPayloadCreator,
    maccommands::SerializableMacCommand,
    parser::{parse_with_factory as lorawan_parse, *},
};
pub enum Session<R>
where
    R: radio::PhyRxTx + Timings,
{
    Idle(Idle<R>),
    SendingData(SendingData<R>),
    WaitingForRxWindow(WaitingForRxWindow<R>),
    WaitingForRx(WaitingForRx<R>),
}

enum RxWindow {
    _1(u32),
    _2(u32),
}

trait SessionState<R: radio::PhyRxTx + Timings> {
    fn get_session(&self) -> &SessionData;
}

macro_rules! into_state {
    ($($from:tt),*) => {
    $(
        impl<R: radio::PhyRxTx + Timings, C: CryptoFactory + Default> From<$from<R>> for Device<R, C>
        {
            fn from(state: $from<R>) -> Device<R, C> {
                Device {
                    crypto: PhantomData::default(),
                    state: SuperState::Session(Session::$from(state))
                }
            }
        }

        impl<R: radio::PhyRxTx + Timings> SessionState<R> for $from<R> {
            fn get_session(&self) -> &SessionData {
                &self.session
            }
        }

        impl<R: radio::PhyRxTx + Timings> CommonState<R> for $from<R> {
            fn get_mut_shared(&mut self) -> &mut Shared<R> {
                &mut self.shared
            }
        }
    )*};
}

impl<R, C> From<Session<R>> for Device<R, C>
where
    R: radio::PhyRxTx + Timings,
    C: CryptoFactory + Default,
{
    fn from(session: Session<R>) -> Device<R, C> {
        Device {
            state: SuperState::Session(session),
            crypto: PhantomData::default(),
        }
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

impl<R> From<Error> for super::super::Error<R>
where
    R: radio::PhyRxTx,
{
    fn from(error: Error) -> super::super::Error<R> {
        super::super::Error::Session(error)
    }
}

impl<R> Session<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn new(shared: Shared<R>, session: SessionData) -> Session<R> {
        Session::Idle(Idle { shared, session })
    }

    pub fn get_mut_shared(&mut self) -> &mut Shared<R> {
        match self {
            Session::Idle(state) => state.get_mut_shared(),
            Session::SendingData(state) => state.get_mut_shared(),
            Session::WaitingForRxWindow(state) => state.get_mut_shared(),
            Session::WaitingForRx(state) => state.get_mut_shared(),
        }
    }

    pub fn get_session_data(&self) -> &SessionData {
        match self {
            Session::Idle(state) => state.get_session(),
            Session::SendingData(state) => state.get_session(),
            Session::WaitingForRxWindow(state) => state.get_session(),
            Session::WaitingForRx(state) => state.get_session(),
        }
    }

    pub fn handle_event<C: CryptoFactory + Default>(
        self,
        event: Event<R>,
    ) -> (Device<R, C>, Result<Response, super::super::Error<R>>) {
        match self {
            Session::Idle(state) => state.handle_event(event),
            Session::SendingData(state) => state.handle_event(event),
            Session::WaitingForRxWindow(state) => state.handle_event(event),
            Session::WaitingForRx(state) => state.handle_event(event),
        }
    }
}

impl<'a, R> Idle<R>
where
    R: radio::PhyRxTx + Timings,
{
    #[allow(clippy::match_wild_err_arm)]
    fn prepare_buffer<C: CryptoFactory + Default>(&mut self, data: &SendData) -> FcntUp {
        let fcnt = self.session.fcnt_up();
        let mut phy: DataPayloadCreator<GenericArray<u8, U256>, C> = DataPayloadCreator::default();
        phy.set_confirmed(data.confirmed)
            .set_f_port(data.fport)
            .set_dev_addr(*self.session.devaddr())
            .set_fcnt(fcnt);

        let mut cmds = Vec::new();
        self.shared.mac.get_cmds(&mut cmds);

        let mut dyn_cmds: Vec<&dyn SerializableMacCommand, U8> = Vec::new();

        for cmd in &cmds {
            if let Err(_e) = dyn_cmds.push(cmd) {
                panic!("dyn_cmds too small compared to cmds")
            }
        }

        match phy.build(
            &data.data,
            dyn_cmds.as_slice(),
            self.session.newskey(),
            self.session.appskey(),
        ) {
            Ok(packet) => {
                self.shared.buffer.clear();
                self.shared.buffer.extend(packet);
            }
            Err(_) => panic!("Error assembling packet!"),
        }
        fcnt
    }
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            Event::SendDataRequest(send_data) => {
                // encodes the packet and places it in send buffer
                let fcnt = self.prepare_buffer::<C>(&send_data);
                let random = (self.shared.get_random)();

                let event: radio::Event<R> = radio::Event::TxRequest(
                    self.shared.region.create_tx_config(
                        random as u8,
                        self.shared.datarate,
                        &Frame::Data,
                    ),
                    &mut self.shared.buffer,
                );

                let confirmed = send_data.confirmed;

                // send the transmit request to the radio
                match self.shared.radio.handle_event(event) {
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
                            radio::Response::TxDone(ms) => {
                                data_rxwindow1_timeout(Session::Idle(self), confirmed, ms)
                            }
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
                let no_session = NoSession::new(self.shared);
                no_session.handle_event(Event::NewSessionRequest)
            }
            Event::RadioEvent(_radio_event) => {
                (self.into(), Err(Error::RadioEventWhileIdle.into()))
            }
        }
    }

    fn into_sending_data(self, confirmed: bool) -> SendingData<R> {
        SendingData {
            session: self.session,
            shared: self.shared,
            confirmed,
        }
    }

    fn into_waiting_for_rxwindow(self, confirmed: bool, time: u32) -> WaitingForRxWindow<R> {
        WaitingForRxWindow {
            session: self.session,
            shared: self.shared,
            rx_window: RxWindow::_1(time),
            confirmed,
        }
    }
}

pub struct Idle<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    session: SessionData,
}

pub struct SendingData<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    session: SessionData,
    confirmed: bool,
}

impl<R> SendingData<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match self.shared.radio.handle_event(radio_event) {
                    Ok(response) => {
                        match response {
                            // expect a complete transmit
                            radio::Response::TxDone(ms) => {
                                let confirmed = self.confirmed;
                                data_rxwindow1_timeout(Session::SendingData(self), confirmed, ms)
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

    fn into_waiting_for_rxwindow(self, confirmed: bool, time: u32) -> WaitingForRxWindow<R> {
        WaitingForRxWindow {
            session: self.session,
            shared: self.shared,
            rx_window: RxWindow::_1(time),
            confirmed,
        }
    }
}

pub struct WaitingForRxWindow<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    session: SessionData,
    confirmed: bool,
    rx_window: RxWindow,
}

impl<'a, R> WaitingForRxWindow<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for a Timeout
            Event::TimeoutFired => {
                let window = match &self.rx_window {
                    RxWindow::_1(_) => Window::_1,
                    RxWindow::_2(_) => Window::_2,
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
                        let window_close: u32 = match self.rx_window {
                            // RxWindow1 one must timeout before RxWindow2
                            RxWindow::_1(time) => {
                                let time_between_windows = self
                                    .shared
                                    .region
                                    .get_rx_delay(&Frame::Data, &Window::_2)
                                    - self.shared.region.get_rx_delay(&Frame::Data, &Window::_1);
                                if time_between_windows
                                    > self.shared.radio.get_rx_window_duration_ms()
                                {
                                    time + self.shared.radio.get_rx_window_duration_ms()
                                } else {
                                    time + time_between_windows
                                }
                            }
                            // RxWindow2 can last however long
                            RxWindow::_2(time) => {
                                time + self.shared.radio.get_rx_window_duration_ms()
                            }
                        };
                        (
                            WaitingForRx::from(self).into(),
                            Ok(Response::TimeoutRequest(window_close)),
                        )
                    }
                    Err(e) => (self.into(), Err(e.into())),
                }
            }
            Event::RadioEvent(_) => (
                self.into(),
                Err(Error::RadioEventWhileWaitingForRxWindow.into()),
            ),
            Event::NewSessionRequest => (
                self.into(),
                Err(Error::NewSessionWhileWaitingForRxWindow.into()),
            ),
            Event::SendDataRequest(_) => (
                self.into(),
                Err(Error::SendDataWhileWaitingForRxWindow.into()),
            ),
        }
    }
}

impl<R> From<WaitingForRxWindow<R>> for WaitingForRx<R>
where
    R: radio::PhyRxTx + Timings,
{
    fn from(val: WaitingForRxWindow<R>) -> WaitingForRx<R> {
        WaitingForRx {
            shared: val.shared,
            session: val.session,
            confirmed: val.confirmed,
            rx_window: val.rx_window,
        }
    }
}

pub struct WaitingForRx<R>
where
    R: radio::PhyRxTx + Timings,
{
    shared: Shared<R>,
    session: SessionData,
    confirmed: bool,
    rx_window: RxWindow,
}

impl<'a, R> WaitingForRx<R>
where
    R: radio::PhyRxTx + Timings,
{
    pub fn handle_event<C: CryptoFactory + Default>(
        mut self,
        event: Event<R>,
    ) -> (Device<R, C>, Result<Response, super::super::Error<R>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match self.shared.radio.handle_event(radio_event) {
                    Ok(response) => match response {
                        radio::Response::RxDone(_quality) => {
                            if let Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) =
                                lorawan_parse(self.shared.radio.get_received_packet(), C::default())
                            {
                                let session = &mut self.session;
                                if session.devaddr() == &encrypted_data.fhdr().dev_addr() {
                                    let fcnt = encrypted_data.fhdr().fcnt() as u32;
                                    if encrypted_data.validate_mic(&session.newskey(), fcnt)
                                        && (fcnt > session.fcnt_down || fcnt == 0)
                                    {
                                        session.fcnt_down = fcnt;
                                        // increment the FcntUp since we have received
                                        // downlink - only reason to not increment
                                        // is if confirmed frame is sent and no
                                        // confirmation (ie: downlink) occurs
                                        session.fcnt_up_increment();

                                        let mut copy = Vec::new();
                                        copy.extend(encrypted_data.as_bytes());

                                        // there two unwraps that are sane in their own right
                                        // * making a new EncryptedDataPayload with owned bytes will
                                        //      always work when copy bytes from another EncryptedPayload
                                        // * the decrypt will always work when we have verified MIC previously
                                        let decrypted = EncryptedDataPayload::new_with_factory(
                                            copy,
                                            C::default(),
                                        )
                                        .unwrap()
                                        .decrypt(
                                            Some(&session.newskey()),
                                            Some(&session.appskey()),
                                            session.fcnt_down,
                                        )
                                        .unwrap();

                                        self.shared.mac.handle_downlink_macs(
                                            &mut self.shared.region,
                                            &mut decrypted.fhdr().fopts(),
                                        );

                                        if let Ok(FRMPayload::MACCommands(mac_cmds)) =
                                            decrypted.frm_payload()
                                        {
                                            self.shared.mac.handle_downlink_macs(
                                                &mut self.shared.region,
                                                &mut mac_cmds.mac_commands(),
                                            );
                                        }

                                        self.shared.downlink =
                                            Some(super::Downlink::Data(decrypted));

                                        // check if FCnt is used up
                                        let response = if self.session.fcnt_up() == (0xFFFF + 1) {
                                            // signal that the session is expired
                                            // client must know to check for potential data
                                            // (FCnt may be extracted when client checks)
                                            Ok(Response::SessionExpired)
                                        } else {
                                            Ok(Response::DownlinkReceived(fcnt))
                                        };
                                        return (self.into_idle().into(), response);
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
                if let Err(_e) = self.shared.radio.handle_event(radio::Event::CancelRx) {
                    panic!("Error cancelling Rx");
                }

                match self.rx_window {
                    RxWindow::_1(t1) => {
                        let time_between_windows =
                            self.shared.region.get_rx_delay(&Frame::Data, &Window::_2)
                                - self.shared.region.get_rx_delay(&Frame::Data, &Window::_1);
                        let t2 = t1 + time_between_windows;
                        // TODO: jump to RxWindow2 if t2 == now
                        (
                            WaitingForRxWindow {
                                shared: self.shared,
                                session: self.session,
                                confirmed: self.confirmed,
                                rx_window: RxWindow::_2(t2),
                            }
                            .into(),
                            Ok(Response::TimeoutRequest(t2)),
                        )
                    }
                    // Timeout during second RxWindow leads to giving up
                    RxWindow::_2(_) => {
                        if !self.confirmed {
                            // if this was not a confirmed frame, we can still
                            // increment the FCnt Up
                            self.session.fcnt_up_increment();
                        }

                        let response = if self.confirmed {
                            // check if FCnt is used up
                            Ok(Response::NoAck)
                        } else if self.session.fcnt_up() == (0xFFFF + 1) {
                            // signal that the session is expired
                            // client must know to check for potential data
                            Ok(Response::SessionExpired)
                        } else {
                            Ok(Response::ReadyToSend)
                        };
                        (self.into_idle().into(), response)
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

    fn into_idle(self) -> Idle<R> {
        Idle {
            shared: self.shared,
            session: self.session,
        }
    }
}

fn data_rxwindow1_timeout<R: radio::PhyRxTx + Timings, C: CryptoFactory + Default>(
    state: Session<R>,
    confirmed: bool,
    timestamp_ms: TimestampMs,
) -> (Device<R, C>, Result<Response, super::super::Error<R>>) {
    let (new_state, first_window) = match state {
        Session::Idle(state) => {
            let first_window = (state.shared.region.get_rx_delay(&Frame::Data, &Window::_1) as i32
                + timestamp_ms as i32
                + state.shared.radio.get_rx_window_offset_ms())
                as u32;
            (
                state.into_waiting_for_rxwindow(confirmed, first_window),
                first_window,
            )
        }
        Session::SendingData(state) => {
            let first_window = (state.shared.region.get_rx_delay(&Frame::Data, &Window::_1) as i32
                + timestamp_ms as i32
                + state.shared.radio.get_rx_window_offset_ms())
                as u32;
            (
                state.into_waiting_for_rxwindow(confirmed, first_window),
                first_window,
            )
        }
        _ => panic!("Invalid state to transition to WaitingForRxWindow"),
    };

    (new_state.into(), Ok(Response::TimeoutRequest(first_window)))
}
