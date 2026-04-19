//! LoRaWAN MAC layer implementation written as a non-async state machine (leveraged by `async_device` and `nb_device`).
//! Manages state internally while providing client with transmit and receive frequencies, while writing to and
//! decrypting from send and receive buffers.

use crate::{
    radio::{self, RadioBuffer, RfConfig, RxConfig, RxMode},
    region, AppSKey, Downlink, NwkSKey,
};
use heapless::Vec;
use lora_modulation::BaseBandModulationParams;
use lorawan::maccommands::SerializableMacCommand;
use lorawan::parser::DevAddr;
use lorawan::types::DR;

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

#[cfg(feature = "certification")]
pub(crate) mod certification;
#[cfg(feature = "multicast")]
pub(crate) mod multicast;

#[derive(Copy, Clone, Debug)]
pub(crate) enum Frame {
    Join,
    Data,
}

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub(crate) enum Window {
    _1,
    _2,
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
/// LoRaWAN Session and Network Configurations
pub struct Configuration {
    pub(crate) data_rate: region::DR,
    pub(crate) rx1_delay: u32,
    join_accept_delay1: u32,
    join_accept_delay2: u32,

    pub(crate) tx_power: Option<u8>,
    pub(crate) rx1_dr_offset: u8,
    pub(crate) rx2_data_rate: Option<DR>,
    pub(crate) rx2_frequency: Option<u32>,
}

pub(crate) struct Mac {
    pub configuration: Configuration,
    pub region: region::Configuration,
    board_eirp: BoardEirp,
    state: State,
    #[cfg(feature = "certification")]
    certification: certification::Certification,
    #[cfg(feature = "multicast")]
    pub multicast: multicast::Multicast,
}

struct BoardEirp {
    max_power: u8,
    antenna_gain: i8,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum State {
    Joined(Session),
    Otaa(otaa::Otaa),
    Unjoined,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error {
    NotJoined,
    #[cfg(feature = "multicast")]
    Multicast(multicast::Error),
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
                rx1_dr_offset: 0,
                join_accept_delay1: region::constants::JOIN_ACCEPT_DELAY1,
                join_accept_delay2: region::constants::JOIN_ACCEPT_DELAY2,
                rx2_data_rate: None,
                rx2_frequency: None,
                tx_power: None,
            },
            #[cfg(feature = "certification")]
            certification: certification::Certification::new(),
            #[cfg(feature = "multicast")]
            multicast: multicast::Multicast::new(),
        }
    }

    /// Prepare the radio buffer with transmitting a join request frame and provides the radio
    /// configuration for the transmission.
    pub(crate) fn join_otaa<RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        credentials: NetworkCredentials,
        buf: &mut RadioBuffer<N>,
    ) -> (radio::TxConfig, u16) {
        let mut otaa = otaa::Otaa::new(credentials);
        let dev_nonce = otaa.prepare_buffer::<RNG, N>(rng, buf);
        self.state = State::Otaa(otaa);
        let mut tx_config =
            self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Join);
        tx_config.adjust_power(self.board_eirp.max_power, self.board_eirp.antenna_gain);
        (tx_config, dev_nonce)
    }

    /// Join via ABP. This does not transmit a join request frame, but instead sets the session.
    pub(crate) fn join_abp(&mut self, nwkskey: NwkSKey, appskey: AppSKey, devaddr: DevAddr) {
        self.state = State::Joined(Session::new(nwkskey, appskey, devaddr));
    }

    /// Join via ABP. This does not transmit a join request frame, but instead sets the session.
    pub(crate) fn set_session(&mut self, session: Session) {
        self.state = State::Joined(session);
    }

    /// Prepare the radio buffer for transmitting a data frame and provide the radio configuration
    /// for the transmission. Returns an error if the device is not joined.
    pub(crate) fn send<RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
        send_data: &SendData<'_>,
    ) -> Result<(radio::TxConfig, FcntUp)> {
        let fcnt = match &mut self.state {
            State::Joined(ref mut session) => Ok(session.prepare_buffer::<N>(send_data, buf)),
            State::Otaa(_) => Err(Error::NotJoined),
            State::Unjoined => Err(Error::NotJoined),
        }?;
        let mut tx_config =
            self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Data);
        tx_config.adjust_power(
            self.configuration.tx_power.unwrap_or(self.board_eirp.max_power),
            self.board_eirp.antenna_gain,
        );
        Ok((tx_config, fcnt))
    }

    pub(crate) fn add_uplink<M: SerializableMacCommand>(&mut self, cmd: M) -> Result<()> {
        let _fcnt = match &mut self.state {
            State::Joined(ref mut session) => {
                session.uplink.add_mac_command(cmd);
                Ok(())
            }
            State::Otaa(_) => Err(Error::NotJoined),
            State::Unjoined => Err(Error::NotJoined),
        };
        Ok(())
    }

    #[cfg(feature = "multicast")]
    pub(crate) fn multicast_setup_send<RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
    ) -> Result<(radio::TxConfig, FcntUp)> {
        self.multicast.setup_send::<N>(&mut self.state, buf).map(|fcnt_up| {
            let mut tx_config =
                self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Data);
            tx_config.adjust_power(
                self.configuration.tx_power.unwrap_or(self.board_eirp.max_power),
                self.board_eirp.antenna_gain,
            );
            (tx_config, fcnt_up)
        })
    }

    #[cfg(feature = "certification")]
    pub(crate) fn certification_setup_send<RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
    ) -> Result<(radio::TxConfig, FcntUp)> {
        self.certification.setup_send::<N>(&mut self.state, buf).map(|fcnt_up| {
            let mut tx_config =
                self.region.create_tx_config(rng, self.configuration.data_rate, &Frame::Data);
            tx_config.adjust_power(self.board_eirp.max_power, self.board_eirp.antenna_gain);
            (tx_config, fcnt_up)
        })
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

    /// Handles a received RF frame. Returns None is unparseable, fails decryption, or fails MIC
    /// verification. Upon successful join, provides Response::JoinSuccess. Upon successful data
    /// rx, provides Response::DownlinkReceived. User must take the downlink from vec for
    /// application data.
    pub(crate) fn handle_rx<const N: usize, const D: usize>(
        &mut self,
        buf: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
        snr: i8,
        rf_config: &RfConfig,
    ) -> Response {
        match &mut self.state {
            State::Joined(ref mut session) => session.handle_rx::<N, D>(
                &mut self.region,
                &mut self.configuration,
                #[cfg(feature = "certification")]
                &mut self.certification,
                #[cfg(feature = "multicast")]
                &mut self.multicast,
                buf,
                dl,
                rf_config.max_payload_len,
                snr,
                false,
            ),
            State::Otaa(ref mut otaa) => {
                if let Some(session) =
                    otaa.handle_rx::<N>(&mut self.region, &mut self.configuration, buf)
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
    #[cfg(feature = "class-c")]
    pub(crate) fn handle_rxc<const N: usize, const D: usize>(
        &mut self,
        buf: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
        snr: i8,
        rf_config: &RfConfig,
    ) -> Result<Response> {
        match &mut self.state {
            State::Joined(ref mut session) => Ok(session.handle_rx::<N, D>(
                &mut self.region,
                &mut self.configuration,
                #[cfg(feature = "certification")]
                &mut self.certification,
                #[cfg(feature = "multicast")]
                &mut self.multicast,
                buf,
                dl,
                rf_config.max_payload_len,
                snr,
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

    /// Build RfConfig for given `Frame` and `Window` and apply
    /// network-specific overrides.
    pub(crate) fn get_rf_config(&self, frame: &Frame, window: &Window) -> RfConfig {
        let (frequency, dr) = match window {
            Window::_1 => (
                self.region.get_rx_frequency(frame, window),
                self.region.get_rx_datarate(
                    self.configuration.data_rate,
                    self.configuration.rx1_dr_offset,
                    window,
                ),
            ),
            Window::_2 => {
                (
                    // RX2 frequency override
                    self.configuration
                        .rx2_frequency
                        .unwrap_or_else(|| self.region.get_rx_frequency(frame, window)),
                    // RX2 datarate override
                    self.configuration.rx2_data_rate.unwrap_or_else(|| {
                        self.region.get_rx_datarate(
                            self.configuration.data_rate,
                            self.configuration.rx1_dr_offset,
                            window,
                        )
                    }),
                )
            }
        };

        // Handle possibly unsupported datarates by falling back to RX2 datarate
        let datarate = match self.region.get_datarate(dr as u8) {
            Some(d) => d,
            None => {
                warn!(
                    "Unsupported DR: {:?} (TX DR: {:?}, Window: {:?})",
                    dr, self.configuration.data_rate, window
                );
                self.region
                    .get_datarate(self.region.get_rx_datarate(
                        self.configuration.data_rate,
                        self.configuration.rx1_dr_offset,
                        &Window::_2,
                    ) as u8)
                    .unwrap()
            }
        };

        RfConfig {
            frequency,
            bb: BaseBandModulationParams::new(
                datarate.spreading_factor,
                datarate.bandwidth,
                self.region.get_coding_rate(),
            ),
            max_payload_len: datarate.max_mac_payload_size,
        }
    }

    pub(crate) fn get_rx_config(&self, buffer_ms: u32, frame: &Frame, window: &Window) -> RxConfig {
        RxConfig { rf: self.get_rf_config(frame, window), mode: RxMode::Single { ms: buffer_ms } }
    }

    #[cfg(feature = "class-c")]
    pub(crate) fn get_rxc_config(&self) -> RxConfig {
        RxConfig { rf: self.get_rf_config(&Frame::Data, &Window::_2), mode: RxMode::Continuous }
    }
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(Debug)]
pub(crate) enum Response {
    NoAck,
    SessionExpired,
    DownlinkReceived(FcntDown),
    NoJoinAccept,
    JoinSuccess,
    NoUpdate,
    RxComplete,
    LinkCheckReq,
    #[cfg(feature = "certification")]
    UplinkPrepared,
    #[cfg(feature = "certification")]
    DeviceHandler(DeviceEvent),
    #[cfg(feature = "multicast")]
    Multicast(multicast::Response),
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(Debug)]
#[cfg(feature = "certification")]
pub(crate) enum DeviceEvent {
    ResetDevice,
    ResetMac,
    TxPeriodicityChange { periodicity: Option<u16> },
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
            Response::LinkCheckReq => unimplemented!(),
            #[cfg(feature = "certification")]
            Response::UplinkPrepared => unimplemented!(),
            #[cfg(feature = "certification")]
            Response::DeviceHandler(_) => unimplemented!(),
            #[cfg(feature = "multicast")]
            Response::Multicast(_) => unimplemented!(),
        }
    }
}

impl From<Response> for async_device::SendResponse {
    fn from(r: Response) -> async_device::SendResponse {
        match r {
            Response::SessionExpired => async_device::SendResponse::SessionExpired,
            Response::DownlinkReceived(fcnt) => async_device::SendResponse::DownlinkReceived(fcnt),
            Response::NoAck => async_device::SendResponse::NoAck,
            Response::RxComplete => async_device::SendResponse::RxComplete,
            #[cfg(feature = "multicast")]
            Response::Multicast(mc) => async_device::SendResponse::Multicast(mc.into()),
            r => panic!("Invalid async_device::SendResponse::from {:?}", r),
        }
    }
}

impl From<Response> for async_device::JoinResponse {
    fn from(r: Response) -> async_device::JoinResponse {
        match r {
            Response::NoJoinAccept => async_device::JoinResponse::NoJoinAccept,
            Response::JoinSuccess => async_device::JoinResponse::JoinSuccess,
            r => panic!("Invalid async_device::JoinResponse::from {:?}", r),
        }
    }
}

impl From<Response> for async_device::ListenResponse {
    fn from(r: Response) -> async_device::ListenResponse {
        match r {
            Response::SessionExpired => async_device::ListenResponse::SessionExpired,
            Response::DownlinkReceived(fcnt) => {
                async_device::ListenResponse::DownlinkReceived(fcnt)
            }
            #[cfg(feature = "multicast")]
            Response::Multicast(mc) => async_device::ListenResponse::Multicast(mc.into()),
            r => panic!("Invalid async_device::ListenResponse::from {:?}", r),
        }
    }
}

fn del_to_delay_ms(del: u8) -> u32 {
    match del {
        2..=15 => del as u32 * 1000,
        _ => region::constants::RECEIVE_DELAY1,
    }
}
