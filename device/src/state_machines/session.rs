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
use super::region::{Frame, Window};
use generic_array::{typenum::U256, GenericArray};
use lorawan::{
    self,
    creator::DataPayloadCreator,
    maccommands::SerializableMacCommand,
    parser::{parse_with_factory as lorawan_parse, *},
};
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
    fn get_session(&self) -> &SessionData;
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
            fn get_session(&self) -> &SessionData {
                &self.session
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

impl<R> From<Error> for super::super::Error<R> {
    fn from(error: Error) -> super::super::Error<R> {
        super::super::Error::Session(error)
    }
}

impl Session {
    pub fn new(session: SessionData) -> Session {
        Session::Idle(Idle { session })
    }

    pub fn get_session_data(&self) -> &SessionData {
        match self {
            Session::Idle(state) => state.get_session(),
            Session::SendingData(state) => state.get_session(),
            Session::WaitingForRxWindow(state) => state.get_session(),
            Session::WaitingForRx(state) => state.get_session(),
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
    ) -> (SuperState, Result<Response, super::super::Error<R::PhyError>>) {
        match self {
            Session::Idle(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            Session::SendingData(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            Session::WaitingForRxWindow(state) => state.handle_event::<R, C, RNG, N>(event, shared),
            Session::WaitingForRx(state) => state.handle_event::<R, C, RNG, N>(event, shared),
        }
    }
}

impl Idle {
    #[allow(clippy::match_wild_err_arm)]
    fn prepare_buffer<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        &mut self,
        data: &SendData,
        shared: &mut Shared<R, RNG, N>,
    ) -> FcntUp {
        let fcnt = self.session.fcnt_up();
        let mut phy: DataPayloadCreator<GenericArray<u8, U256>, C> = DataPayloadCreator::default();

        let mut fctrl = FCtrl(0x0, true);
        if shared.mac.is_confirmed() {
            fctrl.set_ack();
            shared.mac.clear_confirmed();
        }

        phy.set_confirmed(data.confirmed)
            .set_fctrl(&fctrl)
            .set_f_port(data.fport)
            .set_dev_addr(*self.session.devaddr())
            .set_fcnt(fcnt);

        let mut cmds = Vec::new();
        shared.mac.get_cmds(&mut cmds);
        let mut dyn_cmds: Vec<&dyn SerializableMacCommand, 8> = Vec::new();

        for cmd in &cmds {
            if let Err(_e) = dyn_cmds.push(cmd) {
                panic!("dyn_cmds too small compared to cmds")
            }
        }

        match phy.build(
            data.data,
            dyn_cmds.as_slice(),
            self.session.newskey(),
            self.session.appskey(),
        ) {
            Ok(packet) => {
                shared.tx_buffer.clear();
                shared.tx_buffer.extend_from_slice(packet).unwrap();
            }
            Err(e) => panic!("Error assembling packet! {} ", e),
        }
        fcnt
    }
    pub fn handle_event<
        R: radio::PhyRxTx + Timings,
        C: CryptoFactory + Default,
        RNG: RngCore,
        const N: usize,
    >(
        mut self,
        event: Event<R>,
        shared: &mut Shared<R, RNG, N>,
    ) -> (SuperState, Result<Response, super::super::Error<R::PhyError>>) {
        match event {
            Event::SendDataRequest(send_data) => {
                // encodes the packet and places it in send buffer
                let fcnt = self.prepare_buffer::<R, C, RNG, N>(&send_data, shared);
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
        SendingData { session: self.session, confirmed }
    }

    fn into_waiting_for_rxwindow(self, confirmed: bool, time: u32) -> WaitingForRxWindow {
        WaitingForRxWindow { session: self.session, rx_window: RxWindow::_1(time), confirmed }
    }
}

pub struct Idle {
    session: SessionData,
}

pub struct SendingData {
    session: SessionData,
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
        WaitingForRxWindow { session: self.session, rx_window: RxWindow::_1(time), confirmed }
    }
}

pub struct WaitingForRxWindow {
    session: SessionData,
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
        WaitingForRx { session: val.session, confirmed: val.confirmed, rx_window: val.rx_window }
    }
}

pub struct WaitingForRx {
    session: SessionData,
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
    ) -> (SuperState, Result<Response, super::super::Error<R::PhyError>>) {
        match event {
            // we are waiting for the async tx to complete
            Event::RadioEvent(radio_event) => {
                // send the transmit request to the radio
                match shared.radio.handle_event(radio_event) {
                    Ok(response) => match response {
                        radio::Response::RxDone(_quality) => {
                            if let Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) =
                                lorawan_parse(shared.radio.get_received_packet(), C::default())
                            {
                                let session = &mut self.session;
                                if session.devaddr() == &encrypted_data.fhdr().dev_addr() {
                                    let fcnt = encrypted_data.fhdr().fcnt() as u32;
                                    let confirmed = encrypted_data.is_confirmed();
                                    if encrypted_data.validate_mic(session.newskey(), fcnt)
                                        && (fcnt > session.fcnt_down || fcnt == 0)
                                    {
                                        session.fcnt_down = fcnt;
                                        // increment the FcntUp since we have received
                                        // downlink - only reason to not increment
                                        // is if confirmed frame is sent and no
                                        // confirmation (ie: downlink) occurs
                                        session.fcnt_up_increment();

                                        let mut copy = Vec::new();
                                        copy.extend_from_slice(encrypted_data.as_bytes()).unwrap();

                                        // there two unwraps that are sane in their own right
                                        // * making a new EncryptedDataPayload with owned bytes will
                                        //   always work when copy bytes from another
                                        //   EncryptedPayload
                                        // * the decrypt will always work when we have verified MIC
                                        //   previously
                                        let decrypted = EncryptedDataPayload::new_with_factory(
                                            copy,
                                            C::default(),
                                        )
                                        .unwrap()
                                        .decrypt(
                                            Some(session.newskey()),
                                            Some(session.appskey()),
                                            session.fcnt_down,
                                        )
                                        .unwrap();

                                        shared.mac.handle_downlink_macs(
                                            &mut shared.region,
                                            &mut decrypted.fhdr().fopts(),
                                        );
                                        if confirmed {
                                            shared.mac.set_confirmed();
                                        }

                                        if let Ok(FRMPayload::MACCommands(mac_cmds)) =
                                            decrypted.frm_payload()
                                        {
                                            shared.mac.handle_downlink_macs(
                                                &mut shared.region,
                                                &mut mac_cmds.mac_commands(),
                                            );
                                        }

                                        shared.downlink = Some(super::Downlink::Data(decrypted));

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

    fn into_idle(self) -> Idle {
        Idle { session: self.session }
    }
}

fn data_rxwindow1_timeout<R: radio::PhyRxTx + Timings, RNG: RngCore, const N: usize>(
    state: Session,
    confirmed: bool,
    timestamp_ms: TimestampMs,
    shared: &mut Shared<R, RNG, N>,
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
