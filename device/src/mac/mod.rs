use crate::{
    radio::{self, RadioBuffer},
    region, AppSKey, Downlink, NewSKey, RngCore, SendData,
};
use lorawan::parser::DevAddr;
use lorawan::{self, keys::CryptoFactory};

pub type FcntDown = u32;
pub type FcntUp = u32;

mod session;
pub use session::SessionKeys;
mod otaa;
use crate::radio::RfConfig;
pub use otaa::NetworkCredentials;

pub(crate) mod uplink;

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Configuration {
    pub(crate) max_duty_cycle: f32,
    pub(crate) tx_power: Option<u8>,
    pub(crate) tx_data_rate: region::DR,
    pub(crate) rx1_data_rate_offset: Option<u8>,
    pub(crate) rx1_delay: u32,
    pub(crate) rx2_data_rate: Option<region::DR>,
    pub(crate) rx2_frequency: Option<u32>,
    pub(crate) number_of_transmissions: u8,
    pub(crate) join_accept_delay1: u32,
    pub(crate) join_accept_delay2: u32,
}

pub(crate) struct Mac {
    region: region::Configuration,
    state: State,
    pub configuration: Configuration,
}

#[allow(clippy::large_enum_variant)]
enum State {
    Joined(session::Session),
    Otaa(otaa::Otaa),
    Unjoined,
}

#[derive(Debug)]
pub enum Error {
    NotJoined,
    JoinFailed,
}

pub(crate) type Result<T = ()> = core::result::Result<T, Error>;

impl Mac {
    pub(crate) fn new(region: region::Configuration) -> Self {
        Self {
            region,
            state: State::Unjoined,
            configuration: Configuration {
                max_duty_cycle: 1.0,
                tx_power: None,
                tx_data_rate: region::DR::_0,
                rx1_data_rate_offset: None,
                rx1_delay: region::constants::RECEIVE_DELAY1,
                rx2_data_rate: None,
                rx2_frequency: None,
                number_of_transmissions: 1,
                join_accept_delay1: region::constants::JOIN_ACCEPT_DELAY1,
                join_accept_delay2: region::constants::JOIN_ACCEPT_DELAY2,
            },
        }
    }

    /// Prepare the radio buffer with transmitting a join request frame and provides the radio
    /// configuration for the transmission.
    pub(crate) fn join_otaa<C: CryptoFactory + Default, RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        credentials: NetworkCredentials,
        buf: &mut RadioBuffer<N>,
    ) -> radio::TxConfig {
        let mut otaa = otaa::Otaa::new(credentials);
        otaa.prepare_buffer::<C, RNG, N>(rng, buf);
        self.state = State::Otaa(otaa);
        self.region.create_tx_config(rng, self.configuration.tx_data_rate, &region::Frame::Join)
    }

    /// Join via ABP. This does not transmit a join request frame, but instead sets the session.
    pub(crate) fn join_abp(
        &mut self,
        newskey: NewSKey,
        appskey: AppSKey,
        devaddr: DevAddr<[u8; 4]>,
    ) {
        self.state = State::Joined(session::Session::new(newskey, appskey, devaddr));
    }

    /// Prepare the radio buffer for transmitting a data frame and provide the radio configuration
    /// for the transmission. Returns an error if the device is not joined.
    pub(crate) fn send<C: CryptoFactory + Default, RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
        send_data: &SendData,
    ) -> Result<radio::TxConfig> {
        let _fcnt = match &mut self.state {
            State::Joined(ref mut session) => Ok(session.prepare_buffer::<C, N>(send_data, buf)),
            State::Otaa(_) => Err(Error::NotJoined),
            State::Unjoined => Err(Error::NotJoined),
        }?;
        Ok(self.region.create_tx_config(rng, self.configuration.tx_data_rate, &region::Frame::Data))
    }

    pub(crate) fn get_rx_delay(&self, frame: &region::Frame, window: &region::Window) -> u32 {
        match frame {
            region::Frame::Join => match window {
                region::Window::_1 => self.configuration.join_accept_delay1,
                region::Window::_2 => self.configuration.join_accept_delay2,
            },
            region::Frame::Data => match window {
                region::Window::_1 => self.configuration.rx1_delay,
                // RX2 window SHALL be RECEIVE_DELAY1 + 1s
                region::Window::_2 => self.configuration.rx1_delay + 1000,
            },
        }
    }

    /// Gets the radio configuration and timing for a given frame type and window.
    pub(crate) fn get_rx_parameters(
        &mut self,
        frame: &region::Frame,
        window: &region::Window,
    ) -> (RfConfig, u32) {
        (
            self.region.get_rx_config(self.configuration.tx_data_rate, frame, window),
            self.get_rx_delay(frame, window),
        )
    }

    /// Handles a received RF frame. Returns None is unparseable, fails decryption, or fails MIC
    /// verification. Upon successful join, provides Response::JoinSuccess. Upon successful data
    /// rx, provides Response::DownlinkReceived. User must take the radio buffer to parse the
    /// application payload.
    pub(crate) fn handle_rx<C: CryptoFactory + Default>(
        &mut self,
        rx: &mut [u8],
        dl: &mut Option<Downlink>,
    ) -> Option<Response> {
        match &mut self.state {
            State::Joined(ref mut session) => session.handle_rx::<C>(&mut self.region, rx, dl),
            State::Otaa(ref mut otaa) => {
                if let Some(session) = otaa.handle_rx::<C>(&mut self.region, rx) {
                    self.state = State::Joined(session);
                    Some(Response::JoinSuccess)
                } else {
                    None
                }
            }
            State::Unjoined => None,
        }
    }

    pub(crate) fn rx2_complete(&mut self) -> Response {
        match &mut self.state {
            State::Joined(ref mut session) => session.rx2_complete(),
            State::Otaa(_) => Response::NoJoinAccept,
            State::Unjoined => Response::NoUpdate,
        }
    }

    pub(crate) fn get_session_keys(&self) -> Option<SessionKeys> {
        match &self.state {
            State::Joined(session) => session.get_session_keys(),
            State::Otaa(_) => None,
            State::Unjoined => None,
        }
    }

    pub(crate) fn is_joined(&self) -> bool {
        matches!(&self.state, State::Joined(_))
    }

    pub(crate) fn get_fcnt_up(&self) -> Option<FcntUp> {
        match &self.state {
            State::Joined(session) => Some(session.fcnt_up),
            State::Otaa(_) => None,
            State::Unjoined => None,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Response {
    NoAck,
    SessionExpired,
    DownlinkReceived(FcntDown),
    ReadyToSend,
    NoJoinAccept,
    JoinSuccess,
    NoUpdate,
}

impl From<Response> for crate::Response {
    fn from(r: Response) -> Self {
        match r {
            Response::SessionExpired => crate::Response::SessionExpired,
            Response::DownlinkReceived(fcnt) => crate::Response::DownlinkReceived(fcnt),
            Response::NoAck => crate::Response::NoAck,
            Response::ReadyToSend => crate::Response::ReadyToSend,
            Response::NoJoinAccept => crate::Response::NoJoinAccept,
            Response::JoinSuccess => crate::Response::JoinSuccess,
            Response::NoUpdate => crate::Response::NoUpdate,
        }
    }
}

pub fn del_to_delay_ms(del: u8) -> u32 {
    match del {
        2..=15 => del as u32 * 1000,
        _ => region::constants::RECEIVE_DELAY1,
    }
}
