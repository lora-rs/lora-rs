use crate::{
    radio::{self, RadioBuffer},
    region,
    rng::GetRng,
    AppSKey, Downlink, NewSKey, SendData,
};
use lorawan::parser::DevAddr;
use lorawan::{self, keys::CryptoFactory, maccommands::MacCommand};

pub type FcntDown = u32;
pub type FcntUp = u32;

mod session;
pub use session::{Session, SessionKeys};
mod otaa;
use crate::radio::RfConfig;
pub use otaa::NetworkCredentials;

#[cfg(feature = "async")]
use crate::async_device;

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
        cmds: &mut lorawan::maccommands::MacCommandIterator,
    ) {
        use uplink::MacAnsTrait;
        for cmd in cmds {
            match cmd {
                MacCommand::LinkADRReq(payload) => {
                    // we ignore DR and TxPwr
                    region.set_channel_mask(
                        payload.redundancy().channel_mask_control(),
                        payload.channel_mask(),
                    );
                    uplink.adr_ans.add();
                }
                MacCommand::RXTimingSetupReq(payload) => {
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
    state: State,
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

pub(crate) type Result<T = ()> = core::result::Result<T, Error>;

impl Mac {
    pub(crate) fn new(region: region::Configuration) -> Self {
        let data_rate = region.get_default_datarate();
        Self {
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
    pub(crate) fn join_otaa<C: CryptoFactory + Default, RNG: GetRng, const N: usize>(
        &mut self,
        rng: &mut RNG,
        credentials: NetworkCredentials,
        buf: &mut RadioBuffer<N>,
    ) -> (radio::TxConfig, u16) {
        let mut otaa = otaa::Otaa::new(credentials);
        let dev_nonce = otaa.prepare_buffer::<C, RNG, N>(rng, buf);
        self.state = State::Otaa(otaa);
        (self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Join), dev_nonce)
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
    pub(crate) fn send<C: CryptoFactory + Default, RNG: GetRng, const N: usize>(
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
        Ok((self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Data), fcnt))
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
    pub(crate) fn get_rx_parameters(&mut self, frame: &Frame, window: &Window) -> (RfConfig, u32) {
        (
            self.region.get_rx_config(self.configuration.data_rate, frame, window),
            self.get_rx_delay(frame, window),
        )
    }

    /// Handles a received RF frame. Returns None is unparseable, fails decryption, or fails MIC
    /// verification. Upon successful join, provides Response::JoinSuccess. Upon successful data
    /// rx, provides Response::DownlinkReceived. User must take the radio buffer to parse the
    /// application payload.
    pub(crate) fn handle_rx<C: CryptoFactory + Default, const N: usize>(
        &mut self,
        buf: &mut RadioBuffer<N>,
        dl: &mut Option<Downlink>,
    ) -> Response {
        match &mut self.state {
            State::Joined(ref mut session) => {
                session.handle_rx::<C, N>(&mut self.region, &mut self.configuration, buf, dl)
            }
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

impl From<Response> for crate::Response {
    fn from(r: Response) -> Self {
        match r {
            Response::SessionExpired => crate::Response::SessionExpired,
            Response::DownlinkReceived(fcnt) => crate::Response::DownlinkReceived(fcnt),
            Response::NoAck => crate::Response::NoAck,
            Response::NoJoinAccept => crate::Response::NoJoinAccept,
            Response::JoinSuccess => crate::Response::JoinSuccess,
            Response::NoUpdate => crate::Response::NoUpdate,
            Response::RxComplete => crate::Response::RxComplete,
        }
    }
}

#[cfg(feature = "async")]
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

#[cfg(feature = "async")]
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

pub fn del_to_delay_ms(del: u8) -> u32 {
    match del {
        2..=15 => del as u32 * 1000,
        _ => region::constants::RECEIVE_DELAY1,
    }
}
