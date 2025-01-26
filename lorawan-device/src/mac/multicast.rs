use crate::mac;
use crate::radio::RadioBuffer;
use crate::Downlink;
use core::ops::RangeInclusive;
use lorawan::keys::{CryptoFactory, McKEKey};
pub use lorawan::multicast::{self, Session};
use lorawan::multicast::{
    parse_downlink_multicast_messages, DownlinkRemoteSetup, McGroupSetupAnsCreator,
};
use lorawan::parser::FRMPayload;
pub use lorawan::parser::McAddr;
use lorawan::parser::{DataHeader, EncryptedDataPayload};
#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Response {
    NewSession(u8),
    SessionExpired,
    NoUpdate,
    GroupSetupTransmitRequest(u8),
    DownlinkReceived(u32),
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error {}

/// The port used for multicast setup message. The messages are "unicast" and encrypted & sent at
/// the application layer.
const REMOTE_MULTICAST_SETUP_PORT: u8 = 200;
/// These ports are for actual multicast messages; they are encrypted and sent within a multicast
/// session
const DEFAULT_MC_PORT_RANGE: RangeInclusive<u8> = 201..=205;

pub struct Multicast {
    pub(crate) mc_k_e_key: Option<McKEKey>,
    pub(crate) sessions: [Option<Session>; multicast::MAX_GROUPS],
    range: RangeInclusive<u8>,
    remote_setup_port: u8,
    pending_uplinks: Option<heapless::Vec<u8, 256>>,
}

impl Default for Multicast {
    fn default() -> Self {
        Self::new()
    }
}

impl Multicast {
    pub fn new() -> Self {
        Self {
            mc_k_e_key: None,
            range: DEFAULT_MC_PORT_RANGE,
            remote_setup_port: REMOTE_MULTICAST_SETUP_PORT,
            sessions: [None, None, None, None],
            pending_uplinks: None,
        }
    }

    pub(crate) fn handle_rx<C: CryptoFactory + Default, const D: usize>(
        &mut self,
        dl: &mut heapless::Vec<Downlink, D>,
        encrypted_data: EncryptedDataPayload<&mut [u8], C>,
    ) -> Response {
        let mc_addr = encrypted_data.fhdr().mc_addr();
        if let Some(session) = self.matching_session(mc_addr) {
            let fcnt = encrypted_data.fhdr().fcnt() as u32;
            if encrypted_data.validate_mic(session.mc_net_s_key().inner(), fcnt)
                && (fcnt > session.fcnt_down || fcnt == 0)
            {
                return {
                    session.fcnt_down = fcnt;
                    // We can safely unwrap here because we already validated the MIC
                    let decrypted = encrypted_data
                        .decrypt(
                            Some(session.mc_net_s_key().inner()),
                            Some(session.mc_app_s_key().inner()),
                            session.fcnt_down,
                        )
                        .unwrap();
                    if session.fcnt_down == session.max_fcnt_down() {
                        // if the FCnt is used up, the session has expired
                        Response::SessionExpired
                    } else {
                        if let (Some(fport), FRMPayload::Data(data)) =
                            (decrypted.f_port(), decrypted.frm_payload())
                        {
                            // heapless Vec from slice fails only if slice is too large.
                            // A data FRM payload will never exceed 256 bytes.
                            let data = heapless::Vec::from_slice(data).unwrap();
                            // TODO: propagate error when heapless vec is full?
                            let _ = dl.push(Downlink { data, fport });
                        }
                        Response::DownlinkReceived(fcnt)
                    }
                };
            }
        }
        Response::NoUpdate
    }

    /// Sets a custom range for the multicast.
    pub fn set_range(&mut self, range: RangeInclusive<u8>) {
        self.range = range;
    }

    /// Checks if a given port is within the current range.
    pub(crate) fn is_in_range(&self, port: u8) -> bool {
        self.range.contains(&port)
    }

    /// Checks if a given port is the remote multicast setup port
    pub(crate) fn set_remote_setup_port(&mut self, port: u8) {
        self.remote_setup_port = port;
    }

    /// Checks if a given port is the remote multicast setup port
    pub(crate) fn is_remote_setup_port(&self, port: u8) -> bool {
        self.remote_setup_port == port
    }

    pub(crate) fn handle_setup_message<C: CryptoFactory + Default>(
        &mut self,
        data: &[u8],
    ) -> Response {
        if self.mc_k_e_key.is_none() {
            return Response::NoUpdate;
        }
        let mc_k_e_key = self.mc_k_e_key.as_ref().unwrap();
        let messages = parse_downlink_multicast_messages(data);
        for message in messages {
            if let DownlinkRemoteSetup::McGroupSetupReq(mc_group_setup_req) = message {
                let (group_id, session) =
                    mc_group_setup_req.derive_session(&C::default(), mc_k_e_key);
                self.sessions[group_id as usize] = Some(session);
                let mut buffer: heapless::Vec<u8, 256> = heapless::Vec::new();
                let mut ans = McGroupSetupAnsCreator::new();
                ans.mc_group_id_header(group_id);
                buffer.extend_from_slice(ans.build()).unwrap();
                self.pending_uplinks = Some(buffer);
                return Response::GroupSetupTransmitRequest(group_id);
            }
        }
        Response::NoUpdate
    }

    pub(crate) fn setup_send<C: CryptoFactory + Default, const N: usize>(
        &mut self,
        mut state: &mut mac::State,
        buf: &mut RadioBuffer<N>,
    ) -> mac::Result<mac::FcntUp> {
        let send_data = mac::SendData {
            fport: REMOTE_MULTICAST_SETUP_PORT,
            data: self.pending_uplinks.as_ref().unwrap(),
            confirmed: false,
        };
        match &mut state {
            mac::State::Joined(ref mut session) => {
                Ok(session.prepare_buffer::<C, N>(&send_data, buf))
            }
            mac::State::Otaa(_) => Err(mac::Error::NotJoined),
            mac::State::Unjoined => Err(mac::Error::NotJoined),
        }
    }

    pub(crate) fn matching_session(
        &mut self,
        multicast_addr: McAddr<&[u8]>,
    ) -> Option<&mut Session> {
        self.sessions.iter_mut().find_map(|s| {
            if let Some(s) = s {
                if s.multicast_addr() == multicast_addr {
                    return Some(s);
                }
            }
            None
        })
    }
}

impl From<Response> for mac::Response {
    fn from(m: Response) -> Self {
        mac::Response::Multicast(m)
    }
}
