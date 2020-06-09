#![cfg_attr(not(test), no_std)]

use lorawan_encoding::{
    self,
    creator::{DataPayloadCreator, JoinRequestCreator},
    keys::AES128,
    parser::DevAddr,
    parser::{parse as lorawan_parse, JoinAcceptPayload, PhyPayload, EUI64},
};
use core::marker::PhantomData;
use heapless::Vec;
use heapless::consts::*;

pub mod radio;
pub use radio::Radio;
use radio::*;

mod us915;
use us915::Configuration as RegionalConfiguration;

type DevNonce = lorawan_encoding::parser::DevNonce<[u8; 2]>;
type Confirmed = bool;

#[derive(Copy, Clone, Debug)]
pub enum Event {
    StartJoin, // user issued command to start a join process
    TxComplete,
    RxComplete(radio::RxQuality),
    TimerFired,
    SendData(Confirmed),
}

type JoinAttempts = usize;

enum Data {
    NoSession(JoinAttempts, DevNonce),
    Session(Session),
}

type SmHandler<R,E> = fn(&mut Device<R, E>, &mut dyn Radio<Event = E>, Event) -> Option<Response>;

pub struct Device<R: Radio, E> {
    _radio: PhantomData<R>,
    // TODO: do something nicer for randomness
    get_random: fn() -> u32,
    credentials: Credentials,
    region: RegionalConfiguration,
    sm_handler: SmHandler<R,E>,
    sm_data: Data,
    buffer: Vec<u8, U256>,
}

type AppEui = [u8; 8];
type DevEui = [u8; 8];

struct Credentials {
    deveui: DevEui,
    appeui: AppEui,
    appkey: AES128,
}

#[derive(Debug)]
struct Session {
    newskey: AES128,
    appskey: AES128,
    devaddr: DevAddr<[u8; 4]>,
    fcnt: u32,
}

#[derive(Debug, Copy, Clone)]
pub enum Request {
    TimerRequest(usize),
    Error,
}

#[derive(Debug, Copy, Clone)]
pub enum State {
    Joining,
    Joined,
    JoinedFailed,
    JoinedIdle,
    Sending,
    WaitingForWindow,
    InWindow,
    Error
}

#[derive(Debug)]
pub struct Response {
    request: Option<Request>,
    state: State
}

impl Response {
    pub fn request(&self) -> Option<Request>{
        self.request
    }
    pub fn state(&self) -> State {
        self.state
    }
}

impl<R: Radio, E> Device<R, E> {
    pub fn new(
        deveui: [u8; 8],
        appeui: [u8; 8],
        appkey: [u8; 16],
        get_random: fn() -> u32,
    ) -> Device<R, E> {
        let mut region = RegionalConfiguration::new();
        region.set_subband(2);

        Device {
            credentials: Credentials {
                deveui,
                appeui,
                appkey: appkey.into(),
            },
            region,
            get_random,
            _radio: PhantomData::default(),
            sm_handler: Self::not_joined,
            sm_data: Data::NoSession(0, DevNonce::new([0, 0]).unwrap()),
            buffer: Vec::new(),
        }
    }

    pub fn send(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        data: &[u8],
        fport: u8,
        confirmed: bool,
    ) {
        if let Data::Session(session) = &mut self.sm_data {
            let mut phy = DataPayloadCreator::new();
            phy.set_confirmed(confirmed)
                .set_f_port(fport)
                .set_dev_addr(session.devaddr)
                .set_fcnt(session.fcnt);

            match phy.build(&data, &[], &session.newskey, &session.appskey) {
                Ok(packet) => {
                    session.fcnt += 1;
                    self.buffer.clear();
                    self.buffer.extend(packet);
                    (self.sm_handler)(self, radio, Event::SendData(confirmed));
                }
                Err(_output) => {}
            }
        }
    }

    // TODO: no copies
    fn create_join_request(
        &mut self,
        devnonce: u16,
    ) -> DevNonce {
        self.buffer.clear();
        let mut phy = JoinRequestCreator::new();
        let creds = &self.credentials;

        let devnonce = [devnonce as u8, (devnonce >> 8) as u8];
        phy.set_app_eui(EUI64::new(creds.appeui).unwrap())
            .set_dev_eui(EUI64::new(creds.deveui).unwrap())
            .set_dev_nonce(&devnonce);
        let vec = phy.build(&creds.appkey).unwrap();

        let devnonce_ret = DevNonce::new(devnonce).unwrap();
        for el in vec {
            self.buffer.push(*el).unwrap();
        }
        devnonce_ret
    }

    fn send_join_request(&mut self, radio: &mut dyn Radio<Event = E>) -> DevNonce {
        radio.configure_tx(
            14,
            Bandwidth::_125KHZ,
            SpreadingFactor::_10,
            CodingRate::_4_5,
        );

        let mut random = (self.get_random)();
        // use lowest 16 bits for devnonce
        let devnonce = random as u16;
        // we'll use the rest for frequency and subband selection
        random >>= 16;
        radio.set_frequency(self.region.get_join_frequency(random as u8));
        // prepares the buffer
        let devnonce = self.create_join_request(devnonce);
        radio.send(&mut self.buffer);

        devnonce
    }

    fn set_join_accept_rx(&mut self, radio: &mut dyn Radio<Event = E>) {
        radio.configure_rx(Bandwidth::_500KHZ, SpreadingFactor::_10, CodingRate::_4_5);
        radio.set_frequency(self.region.get_join_accept_frequency1());
        radio.set_rx();
    }

    fn set_rxwindow1(&mut self, radio: &mut dyn Radio<Event = E>) {
        radio.configure_rx(Bandwidth::_500KHZ, SpreadingFactor::_10, CodingRate::_4_5);
        radio.set_frequency(self.region.get_rxwindow1_frequency());
        radio.set_rx();
    }

    pub fn handle_radio_event(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        event: E,
    ) -> Option<Response> {
        let radio_state = radio.handle_event(event);

        match radio_state {
            radio::State::Busy => None,
            radio::State::TxDone => self.handle_event(radio, Event::TxComplete),
            radio::State::RxDone(quality) => self.handle_event(radio, Event::RxComplete(quality)),
            radio::State::TxError => None,
            radio::State::RxError => None,
        }
    }

    pub fn handle_event(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        event: Event,
    ) -> Option<Response> {
        (self.sm_handler)(self, radio, event)
    }

    // BELOW HERE ARE PRIVATE STATE MACHINE HANDLERS
    fn error(&mut self, _radio: &mut dyn Radio<Event = E>, _event: Event) -> Option<Response> {
        // can do a richer implementation later
        Some(Response {
            request: Some(Request::Error),
            state: State::Error
        })
    }

    fn not_joined(&mut self, radio: &mut dyn Radio<Event = E>, event: Event) -> Option<Response> {
        match event {
            Event::StartJoin => {
                if let Data::NoSession(attempts, _) = self.sm_data {
                    self.sm_handler = Device::join_sent;
                    let devnonce = self.send_join_request(radio);
                    self.sm_data = Data::NoSession(attempts + 1, devnonce);
                    None
                } else {
                    self.error(radio, event)
                }
            }
            _ => self.error(radio, event),
        }
    }

    fn join_sent(&mut self, radio: &mut dyn Radio<Event = E>, event: Event) -> Option<Response> {
        match event {
            Event::TxComplete => {
                self.sm_handler = Device::waiting_join_delay1;
                let mut timeout: isize = (self.region.get_join_accept_delay1() * 1000) as isize;
                timeout += radio.get_rx_window_offset_ms();
                Some(Response {
                    request: Some(Request::TimerRequest(
                        // TODO: determine this error adjustment more scientifically
                        timeout as usize
                    )),
                    state: State::WaitingForWindow
                })
            }
            _ => self.error(radio, event),
        }
    }

    fn waiting_join_delay1(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        event: Event,
    ) -> Option<Response> {
        match event {
            Event::TimerFired => {
                self.sm_handler = Device::waiting_join_accept1;
                self.set_join_accept_rx(radio);
                Some(Response {
                    request: Some(Request::TimerRequest(
                        // TODO: handle situation where duration is longer than next window
                        radio.get_rx_window_duration_ms(),
                    )),
                    state: State::InWindow
                })
            }
            _ => self.error(radio, event),
        }
    }

    fn waiting_join_accept1(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        event: Event,
    ) -> Option<Response> {
        match event {
            Event::RxComplete(_quality) => {
                if let Data::NoSession(_, devnonce) = self.sm_data {
                    let packet = lorawan_parse(radio.get_received_packet()).unwrap();

                    if let PhyPayload::JoinAccept(join_accept) = packet {
                        if let JoinAcceptPayload::Encrypted(encrypted) = join_accept {
                            let decrypt = encrypted.decrypt(&self.credentials.appkey);
                            if decrypt.validate_mic(&self.credentials.appkey) {
                                let session = Session {
                                    newskey: decrypt
                                        .derive_newskey(&devnonce, &self.credentials.appkey),
                                    appskey: decrypt
                                        .derive_appskey(&devnonce, &self.credentials.appkey),
                                    devaddr: DevAddr::new([
                                        decrypt.dev_addr().as_ref()[0],
                                        decrypt.dev_addr().as_ref()[1],
                                        decrypt.dev_addr().as_ref()[2],
                                        decrypt.dev_addr().as_ref()[3],
                                    ])
                                    .unwrap(),
                                    fcnt: 0,
                                };
                                self.sm_handler = Device::joined_idle;
                                self.sm_data = Data::Session(session);

                                return Some(Response {
                                    request: None,
                                    state: State::JoinedIdle
                                });
                            } else {

                            }
                        } else {
                            panic!("Cannot possibly be decrypted already")
                        }
                    } else {
                        // just some other packet, ignore
                    }
                    None
                } else {
                    self.error(radio, event)
                }
            }
            _ => self.error(radio, event),
        }
    }

    fn joined_idle(&mut self, radio: &mut dyn Radio<Event = E>, event: Event) -> Option<Response> {
        if let Data::Session(_) = self.sm_data {
            match event {
                Event::SendData(_) => {
                    radio.configure_tx(
                        14,
                        Bandwidth::_125KHZ,
                        SpreadingFactor::_10,
                        CodingRate::_4_5,
                    );
                    let random = (self.get_random)();
                    radio.set_frequency(self.region.get_data_frequency(random as u8));
                    radio.send(&mut self.buffer);
                    self.sm_handler = Device::joined_sending;

                    None
                }
                _ => self.error(radio, event),
            }
        } else {
            self.error(radio, event)
        }
    }

    fn joined_sending(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        event: Event,
    ) -> Option<Response> {
        match event {
            Event::TxComplete => {
                self.sm_handler = Device::joined_waiting_rxwindow1;
                let mut timeout: isize = (self.region.get_receive_delay1() * 1000) as isize;
                timeout += radio.get_rx_window_offset_ms();
                Some(Response {
                    request: Some(Request::TimerRequest(
                        // TODO: determine this error adjustment more scientifically
                        timeout as usize
                    )),
                    state: State::WaitingForWindow
                })
            }
            _ => self.error(radio, event),
        }
    }

    fn joined_waiting_rxwindow1(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        event: Event,
    ) -> Option<Response> {
        match event {
            Event::TimerFired => {
                self.sm_handler = Device::joined_rxwindow1;
                self.set_rxwindow1(radio);
                Some(Response {
                    request: Some(Request::TimerRequest(
                        // TODO: handle situation where duration is longer than next window
                        radio.get_rx_window_duration_ms(),
                    )),
                    state: State::InWindow
                })
            }
            _ => self.error(radio, event),
        }
    }

    fn joined_rxwindow1(
        &mut self,
        radio: &mut dyn Radio<Event = E>,
        event: Event,
    ) -> Option<Response> {
        match event {
            Event::RxComplete(_quality) => {
                if let Data::Session(session) = &self.sm_data {

                }
                self.sm_handler = Device::joined_idle;
                Some(Response {
                    request: None,
                    state: State::JoinedIdle
                })

            }
            _ => self.error(radio, event),
        }
    }
}
