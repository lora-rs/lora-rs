use lorawan_encoding::keys::AES128;

pub type AppEui = [u8; 8];
pub type DevEui = [u8; 8];

#[derive(Debug)]
pub struct Credentials {
    deveui: DevEui,
    appeui: AppEui,
    appkey: AES128,
}

impl Credentials {
    pub fn new(appeui: AppEui, deveui: DevEui, appkey: [u8; 16]) -> Credentials {
        Credentials {
            deveui,
            appeui,
            appkey: appkey.into(),
        }
    }

    pub fn appeui(&self) -> &AppEui {
        &self.appeui
    }

    pub fn deveui(&self) -> &DevEui {
        &self.deveui
    }

    pub fn appkey(&self) -> &AES128 {
        &self.appkey
    }
}

pub struct SessionKeys {
    newskey: AES128,
    appskey: AES128,
    devaddr: u32,
}

use super::state_machines::no_session::SessionData;

impl SessionKeys {
    pub fn copy_from_session_data(session_data: &SessionData) -> SessionKeys {
        let session_devaddr = session_data.devaddr().as_ref();
        let devaddr = (session_devaddr[3] as u32)
            | (session_devaddr[2] as u32) << 8
            | (session_devaddr[1] as u32) << 16
            | (session_devaddr[0] as u32) << 24;
        SessionKeys {
            newskey: *session_data.newskey(),
            appskey: *session_data.appskey(),
            devaddr,
        }
    }
}
use core::fmt;
impl core::fmt::Debug for SessionKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionKeys {{ NewSKey: {:?}, AppsSKey: {:?}, DevAddr {:x}}}",
            self.newskey, self.appskey, self.devaddr
        )
    }
}
