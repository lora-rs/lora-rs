use crate::mac::{AppSKey, NewSKey};
pub use lorawan::parser::MulticastAddr;

pub(crate) type Result<T = ()> = core::result::Result<T, Error>;

const DEFAULT_MC_PORT: u8 = 200;

#[derive(Debug)]
pub enum Error {
    NoAvailableSlotForSession,
}

pub(crate) struct Multicast {
    port: u8,
    pub sessions: [Option<Session>; 4],
}

impl Multicast {
    pub fn new() -> Self {
        Self { port: DEFAULT_MC_PORT, sessions: [None, None, None, None] }
    }
    pub(crate) fn set_port(&mut self, port: u8) {
        self.port = port;
    }

    pub(crate) fn port(&self) -> u8 {
        self.port
    }

    pub(crate) fn matching_session(&mut self, multicast_addr: MulticastAddr<&[u8]>) -> Option<&mut Session> {
        self.sessions.iter_mut().find_map(|s| {
            if let Some(s) = s {
                println!("s.multicast_addr(): = {:?} =? multicast_addr: {:?}", s.multicast_addr(),  multicast_addr);
                if s.multicast_addr() == multicast_addr {
                    return Some(s);
                }
            }
            None
        })
    }

    pub(crate) fn add_session(
        &mut self,
        multicast_addr: MulticastAddr<[u8; 4]>,
        newskey: NewSKey,
        appskey: AppSKey,
    ) -> Result {
        for i in 0..self.sessions.len() {
            if self.sessions[i].is_none() {
                self.sessions[i] = Some(Session { multicast_addr, newskey, appskey, fcnt_down: 0 });
                return Ok(());
            }
        }
        Err(Error::NoAvailableSlotForSession)
    }
}


#[derive(Debug)]
pub(crate) struct Session {
    multicast_addr: MulticastAddr<[u8; 4]>,
    newskey: NewSKey,
    appskey: AppSKey,
    pub(crate) fcnt_down: u32,
}

impl Session {
    pub(crate) fn multicast_addr(&self) -> MulticastAddr<[u8; 4]> {
        self.multicast_addr
    }
    pub(crate) fn newskey(&self) -> NewSKey {
        self.newskey
    }
    pub(crate) fn appskey(&self) -> AppSKey {
        self.appskey
    }
}

