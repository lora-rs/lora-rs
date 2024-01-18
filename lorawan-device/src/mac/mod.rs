//! LoRaWAN MAC layer implementation written as a non-async state machine (leveraged by `async_device` and `nb_device`).
//! Manages state internally while providing client with transmit and receive frequencies, while writing to and
//! decrypting from send and receive buffers.

use crate::{
    radio::{self, RadioBuffer, RfConfig, RxConfig, RxMode},
    region, AppSKey, Downlink, NewSKey,
};
use heapless::Vec;
use lorawan::{self, keys::CryptoFactory};
use lorawan::{maccommands::DownlinkMacCommand, parser::DevAddr};

pub type FcntDown = u32;
pub type FcntUp = u32;

mod session;
use rand_core::RngCore;
pub use session::{Session, SessionKeys};

mod otaa;
pub use otaa::NetworkCredentials;

use crate::async_device;
use crate::nb_device;

pub(crate) mod uplink;

#[derive(Copy, Clone, Debug)]
pub(crate) enum Frame {
    Join,
    Data,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum Window {
    _1,
    _2,
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// LoRaWAN Session and Network Configurations
pub struct Configuration {
    pub(crate) data_rate: region::DR,
    rx1_delay: u32,
    join_accept_delay1: u32,
    join_accept_delay2: u32,
}

impl Configuration {
    fn handle_downlink_macs(
        &mut self,
        region: &mut region::Configuration,
        uplink: &mut uplink::Uplink,
        cmds: lorawan::maccommands::MacCommandIterator<DownlinkMacCommand>,
    ) {
        use uplink::MacAnsTrait;
        for cmd in cmds {
            match cmd {
                DownlinkMacCommand::LinkADRReq(payload) => {
                    // we ignore DR and TxPwr
                    region.set_channel_mask(
                        payload.redundancy().channel_mask_control(),
                        payload.channel_mask(),
                    );
                    uplink.adr_ans.add();
                }
                DownlinkMacCommand::RXTimingSetupReq(payload) => {
                    self.rx1_delay = del_to_delay_ms(payload.delay());
                    uplink.ack_rx_delay();
                }
                _ => (),
            }
        }
    }
}

pub(crate) struct Mac {
    pub configuration: Configuration,
    pub region: region::Configuration,
    board_eirp: BoardEirp,
    state: State,
}

struct BoardEirp {
    max_power: u8,
    antenna_gain: i8,
}

#[allow(clippy::large_enum_variant)]
enum State {
    Joined(Session),
    Otaa(otaa::Otaa),
    Unjoined,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    NotJoined,
    InvalidResponse(Response),
}

pub struct SendData<'a> {
    pub data: &'a [u8],
    pub fport: u8,
    pub confirmed: bool,
}

pub(crate) type Result<T = ()> = core::result::Result<T, Error>;

impl Mac {
    pub(crate) fn new(region: region::Configuration, max_power: u8, antenna_gain: i8) -> Self {
        let data_rate = region.get_default_datarate();
        Self {
            board_eirp: BoardEirp { max_power, antenna_gain },
            region,
            state: State::Unjoined,
            configuration: Configuration {
                data_rate,
                rx1_delay: region::constants::RECEIVE_DELAY1,
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
    ) -> (radio::TxConfig, u16) {
        let mut otaa = otaa::Otaa::new(credentials);
        let dev_nonce = otaa.prepare_buffer::<C, RNG, N>(rng, buf);
        self.state = State::Otaa(otaa);
        let mut tx_config =
            self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Join);
        tx_config.adjust_power(self.board_eirp.max_power, self.board_eirp.antenna_gain);
        (tx_config, dev_nonce)
    }

    /// Join via ABP. This does not transmit a join request frame, but instead sets the session.
    pub(crate) fn join_abp(
        &mut self,
        newskey: NewSKey,
        appskey: AppSKey,
        devaddr: DevAddr<[u8; 4]>,
    ) {
        self.state = State::Joined(Session::new(newskey, appskey, devaddr));
    }

    /// Join via ABP. This does not transmit a join request frame, but instead sets the session.
    pub(crate) fn set_session(&mut self, session: Session) {
        self.state = State::Joined(session);
    }

    /// Prepare the radio buffer for transmitting a data frame and provide the radio configuration
    /// for the transmission. Returns an error if the device is not joined.
    pub(crate) fn send<C: CryptoFactory + Default, RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
        send_data: &SendData,
    ) -> Result<(radio::TxConfig, FcntUp)> {
        let fcnt = match &mut self.state {
            State::Joined(ref mut session) => Ok(session.prepare_buffer::<C, N>(send_data, buf)),
            State::Otaa(_) => Err(Error::NotJoined),
            State::Unjoined => Err(Error::NotJoined),
        }?;
        let mut tx_config =
            self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Data);
        tx_config.adjust_power(self.board_eirp.max_power, self.board_eirp.antenna_gain);
        Ok((tx_config, fcnt))
    }

    pub(crate) fn get_rx_delay(&self, frame: &Frame, window: &Window) -> u32 {
        match frame {
            Frame::Join => match window {
                Window::_1 => self.configuration.join_accept_delay1,
                Window::_2 => self.configuration.join_accept_delay2,
            },
            Frame::Data => match window {
                Window::_1 => self.configuration.rx1_delay,
                // RECEIVE_DELAY2 is not configurable. LoRaWAN 1.0.3 Section 5.7:
                // "The second reception slot opens one second after the first reception slot."
                Window::_2 => self.configuration.rx1_delay + 1000,
            },
        }
    }

    /// Gets the radio configuration and timing for a given frame type and window.
    pub(crate) fn get_rx_parameters_legacy(
        &mut self,
        frame: &Frame,
        window: &Window,
    ) -> (RfConfig, u32) {
        (
            self.region.get_rx_config(self.configuration.data_rate, frame, window),
            self.get_rx_delay(frame, window),
        )
    }

    /// Handles a received RF frame. Returns None is unparseable, fails decryption, or fails MIC
    /// verification. Upon successful join, provides Response::JoinSuccess. Upon successful data
    /// rx, provides Response::DownlinkReceived. User must take the downlink from vec for
    /// application data.
    pub(crate) fn handle_rx<C: CryptoFactory + Default, const N: usize, const D: usize>(
        &mut self,
        buf: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
    ) -> Response {
        match &mut self.state {
            State::Joined(ref mut session) => session.handle_rx::<C, N, D>(
                &mut self.region,
                &mut self.configuration,
                buf,
                dl,
                false,
            ),
            State::Otaa(ref mut otaa) => {
                if let Some(session) =
                    otaa.handle_rx::<C, N>(&mut self.region, &mut self.configuration, buf)
                {
                    self.state = State::Joined(session);
                    Response::JoinSuccess
                } else {
                    Response::NoUpdate
                }
            }
            State::Unjoined => Response::NoUpdate,
        }
    }

    /// Handles a received RF frame during RXC window. Returns None if unparseable, fails decryption,
    /// or fails MIC verification. Upon successful data rx, provides Response::DownlinkReceived.
    /// User must later call `take_downlink()` on the device to get the application data.
    pub(crate) fn handle_rxc<C: CryptoFactory + Default, const N: usize, const D: usize>(
        &mut self,
        buf: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
    ) -> Result<Response> {
        match &mut self.state {
            State::Joined(ref mut session) => Ok(session.handle_rx::<C, N, D>(
                &mut self.region,
                &mut self.configuration,
                buf,
                dl,
                true,
            )),
            State::Otaa(_) => Err(Error::NotJoined),
            State::Unjoined => Err(Error::NotJoined),
        }
    }

    pub(crate) fn rx2_complete(&mut self) -> Response {
        match &mut self.state {
            State::Joined(session) => session.rx2_complete(),
            State::Otaa(otaa) => otaa.rx2_complete(),
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

    pub(crate) fn get_session(&self) -> Option<&Session> {
        match &self.state {
            State::Joined(session) => Some(session),
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

    pub(crate) fn get_rx_config(&self, buffer_ms: u32, frame: &Frame, window: &Window) -> RxConfig {
        RxConfig {
            rf: self.region.get_rx_config(self.configuration.data_rate, frame, window),
            mode: RxMode::Single { ms: buffer_ms },
        }
    }

    pub(crate) fn get_rxc_config(&self) -> RxConfig {
        RxConfig {
            rf: self.region.get_rxc_config(self.configuration.data_rate),
            mode: RxMode::Continuous,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub enum Response {
    NoAck,
    SessionExpired,
    DownlinkReceived(FcntDown),
    NoJoinAccept,
    JoinSuccess,
    NoUpdate,
    RxComplete,
}

impl From<Response> for nb_device::Response {
    fn from(r: Response) -> Self {
        match r {
            Response::SessionExpired => nb_device::Response::SessionExpired,
            Response::DownlinkReceived(fcnt) => nb_device::Response::DownlinkReceived(fcnt),
            Response::NoAck => nb_device::Response::NoAck,
            Response::NoJoinAccept => nb_device::Response::NoJoinAccept,
            Response::JoinSuccess => nb_device::Response::JoinSuccess,
            Response::NoUpdate => nb_device::Response::NoUpdate,
            Response::RxComplete => nb_device::Response::RxComplete,
        }
    }
}

impl TryFrom<Response> for async_device::SendResponse {
    type Error = Error;

    fn try_from(r: Response) -> Result<async_device::SendResponse> {
        match r {
            Response::SessionExpired => Ok(async_device::SendResponse::SessionExpired),
            Response::DownlinkReceived(fcnt) => {
                Ok(async_device::SendResponse::DownlinkReceived(fcnt))
            }
            Response::NoAck => Ok(async_device::SendResponse::NoAck),
            Response::RxComplete => Ok(async_device::SendResponse::RxComplete),
            r => Err(Error::InvalidResponse(r)),
        }
    }
}

impl TryFrom<Response> for async_device::JoinResponse {
    type Error = Error;

    fn try_from(r: Response) -> Result<async_device::JoinResponse> {
        match r {
            Response::NoJoinAccept => Ok(async_device::JoinResponse::NoJoinAccept),
            Response::JoinSuccess => Ok(async_device::JoinResponse::JoinSuccess),
            r => Err(Error::InvalidResponse(r)),
        }
    }
}

fn del_to_delay_ms(del: u8) -> u32 {
    match del {
        2..=15 => del as u32 * 1000,
        _ => region::constants::RECEIVE_DELAY1,
    }
}
