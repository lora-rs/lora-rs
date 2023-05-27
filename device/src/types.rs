use crate::radio::{RadioBuffer, TxConfig};
use crate::region::{Configuration, Frame, DR};
use crate::GetRandom;
use lorawan::keys::CryptoFactory;
use lorawan::{creator::JoinRequestCreator, keys::AES128, parser::EUI64};

pub type AppEui = [u8; 8];
pub type DevEui = [u8; 8];

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub struct Credentials {
    deveui: DevEui,
    appeui: AppEui,
    appkey: AES128,
}

pub(crate) type DevNonce = lorawan::parser::DevNonce<[u8; 2]>;

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

    /// Prepare a join request to be sent. This populates the radio buffer with the request to be
    /// sent, and returns the radio config to use for transmitting.
    ///
    /// # Note
    ///
    /// This method requires that the RNG buffer hold at least 2 random bytes.
    pub(crate) fn create_join_request<
        C: CryptoFactory + Default,
        RNG: GetRandom,
        const N: usize,
    >(
        &self,
        region: &mut Configuration,
        rng: &mut RNG,
        datarate: DR,
        buf: &mut RadioBuffer<N>,
    ) -> (DevNonce, TxConfig) {
        // Use lowest 16 bits for devnonce
        // Unwrapping is OK: if a panic occurs, that's because the RNG buffer isn't full enough.
        let devnonce_bytes = rng.get_random().unwrap().into_u16_truncate();

        buf.clear();

        let mut phy: JoinRequestCreator<[u8; 23], C> = JoinRequestCreator::default();

        let devnonce = [devnonce_bytes as u8, (devnonce_bytes >> 8) as u8];

        phy.set_app_eui(EUI64::new(self.appeui()).unwrap())
            .set_dev_eui(EUI64::new(self.deveui()).unwrap())
            .set_dev_nonce(&devnonce);
        let vec = phy.build(self.appkey()).unwrap();

        let devnonce_copy = DevNonce::new(devnonce).unwrap();

        // Unwrapping here because if an error occurs, that's because the RNG buffer isn't full enough, and that's a
        // programming error.
        let tx_config = region
            .create_tx_config(rng, datarate, &Frame::Join)
            .unwrap();

        buf.extend_from_slice(vec).unwrap();
        (devnonce_copy, tx_config)
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
