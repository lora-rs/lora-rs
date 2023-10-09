use super::session::Session;
use crate::radio::{RadioBuffer, TxConfig};
use crate::region::{Configuration, Frame, DR};
use crate::RngCore;
use crate::{AppEui, AppKey, DevEui};
use lorawan::keys::CryptoFactory;
use lorawan::{
    creator::JoinRequestCreator,
    parser::{parse_with_factory as lorawan_parse, *},
};

pub(crate) type DevNonce = lorawan::parser::DevNonce<[u8; 2]>;

pub(crate) struct Otaa {
    dev_nonce: DevNonce,
    network_credentials: NetworkCredentials,
}
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub struct NetworkCredentials {
    deveui: DevEui,
    appeui: AppEui,
    appkey: AppKey,
}

impl Otaa {
    pub fn new(network_credentials: NetworkCredentials) -> Self {
        Self { dev_nonce: DevNonce::from([0, 0]), network_credentials }
    }

    /// Prepare a join request to be sent. This populates the radio buffer with the request to be
    /// sent, and returns the radio config to use for transmitting.
    pub(crate) fn prepare_buffer<C: CryptoFactory + Default, RNG: RngCore, const N: usize>(
        &mut self,
        rng: &mut RNG,
        buf: &mut RadioBuffer<N>,
    ) {
        self.dev_nonce = DevNonce::from(rng.next_u32() as u16);

        buf.clear();
        let mut phy: JoinRequestCreator<[u8; 23], C> = JoinRequestCreator::default();

        phy.set_app_eui(self.network_credentials.appeui.0)
            .set_dev_eui(self.network_credentials.deveui.0)
            .set_dev_nonce(self.dev_nonce);
        let vec = phy.build(&self.network_credentials.appkey.0).unwrap();
        buf.extend_from_slice(vec).unwrap();
    }

    pub(crate) fn handle_rx<C: CryptoFactory + Default>(
        &mut self,
        region: &mut Configuration,
        rx: &mut [u8],
    ) -> Option<Session> {
        if let Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(encrypted))) =
            lorawan_parse(rx, C::default())
        {
            let decrypt = encrypted.decrypt(&self.network_credentials.appkey.0);
            region.process_join_accept(&decrypt);
            if decrypt.validate_mic(&self.network_credentials.appkey.0) {
                return Some(Session::derive_new(
                    &decrypt,
                    self.dev_nonce,
                    &self.network_credentials,
                ));
            }
        }
        None
    }
}

/// TODO: remove
/// We maintain this impl for now until async_device is ready to use the complete mac struct
impl NetworkCredentials {
    pub fn new(appeui: AppEui, deveui: DevEui, appkey: AppKey) -> Self {
        Self { deveui, appeui, appkey }
    }
    pub fn appeui(&self) -> &AppEui {
        &self.appeui
    }

    pub fn deveui(&self) -> &DevEui {
        &self.deveui
    }

    pub fn appkey(&self) -> &AppKey {
        &self.appkey
    }

    /// Prepare a join request to be sent. This populates the radio buffer with the request to be
    /// sent, and returns the radio config to use for transmitting.
    pub(crate) fn create_join_request<C: CryptoFactory + Default, RNG: RngCore, const N: usize>(
        &self,
        region: &mut Configuration,
        rng: &mut RNG,
        datarate: DR,
        buf: &mut RadioBuffer<N>,
    ) -> (DevNonce, TxConfig) {
        // use lowest 16 bits for devnonce
        let devnonce_bytes = rng.next_u32() as u16;

        buf.clear();

        let mut phy: JoinRequestCreator<[u8; 23], C> = JoinRequestCreator::default();

        let devnonce = [devnonce_bytes as u8, (devnonce_bytes >> 8) as u8];

        phy.set_app_eui(self.appeui().0).set_dev_eui(self.deveui().0).set_dev_nonce(&devnonce);
        let vec = phy.build(&self.appkey().0).unwrap();

        let devnonce_copy = DevNonce::new(devnonce).unwrap();

        buf.extend_from_slice(vec).unwrap();
        (devnonce_copy, region.create_tx_config(rng, datarate, &Frame::Join))
    }
}
