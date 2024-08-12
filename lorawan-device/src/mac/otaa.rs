use super::{del_to_delay_ms, session::Session, Response};
use crate::radio::RadioBuffer;
use crate::region::Configuration;
use crate::{AppEui, AppKey, DevEui};
use lorawan::keys::CryptoFactory;
use lorawan::{
    creator::JoinRequestCreator,
    parser::{parse_with_factory as lorawan_parse, *},
};
use rand_core::RngCore;

pub(crate) type DevNonce = lorawan::parser::DevNonce<[u8; 2]>;

pub(crate) struct Otaa {
    dev_nonce: DevNonce,
    network_credentials: NetworkCredentials,
}
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
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
    pub(crate) fn prepare_buffer<C: CryptoFactory + Default, G: RngCore, const N: usize>(
        &mut self,
        rng: &mut G,
        buf: &mut RadioBuffer<N>,
    ) -> u16 {
        self.dev_nonce = DevNonce::from(rng.next_u32() as u16);
        buf.clear();
        let mut phy: JoinRequestCreator<[u8; 23], C> = JoinRequestCreator::default();
        phy.set_app_eui(self.network_credentials.appeui)
            .set_dev_eui(self.network_credentials.deveui)
            .set_dev_nonce(self.dev_nonce);
        let vec = phy.build(&self.network_credentials.appkey);
        buf.extend_from_slice(vec).unwrap();
        u16::from(self.dev_nonce)
    }

    pub(crate) fn handle_rx<C: CryptoFactory + Default, const N: usize>(
        &mut self,
        region: &mut Configuration,
        configuration: &mut super::Configuration,
        rx: &mut RadioBuffer<N>,
    ) -> Option<Session> {
        if let Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(encrypted))) =
            lorawan_parse(rx.as_mut_for_read(), C::default())
        {
            let decrypt = encrypted.decrypt(&self.network_credentials.appkey);
            region.process_join_accept(&decrypt);
            configuration.rx1_delay = del_to_delay_ms(decrypt.rx_delay());
            if decrypt.validate_mic(&self.network_credentials.appkey) {
                return Some(Session::derive_new(
                    &decrypt,
                    self.dev_nonce,
                    &self.network_credentials,
                ));
            }
        }
        None
    }

    pub(crate) fn rx2_complete(&mut self) -> Response {
        Response::NoJoinAccept
    }
}

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
}
