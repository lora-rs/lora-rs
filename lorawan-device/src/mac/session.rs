use crate::{region, AppSKey, Downlink, NewSKey};
use heapless::Vec;
use lorawan::keys::CryptoFactory;
use lorawan::{
    creator::DataPayloadCreator,
    maccommands::SerializableMacCommand,
    parser::{parse_with_factory as lorawan_parse, *},
    parser::{DecryptedJoinAcceptPayload, DevAddr},
};

use generic_array::{typenum::U256, GenericArray};

use crate::radio::RadioBuffer;

use super::{
    otaa::{DevNonce, NetworkCredentials},
    uplink, FcntUp, Response, SendData,
};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Session {
    pub uplink: uplink::Uplink,
    pub confirmed: bool,
    pub newskey: NewSKey,
    pub appskey: AppSKey,
    pub devaddr: DevAddr<[u8; 4]>,
    pub fcnt_up: u32,
    pub fcnt_down: u32,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SessionKeys {
    pub newskey: NewSKey,
    pub appskey: AppSKey,
    pub devaddr: DevAddr<[u8; 4]>,
}

impl From<Session> for SessionKeys {
    fn from(session: Session) -> Self {
        Self { newskey: session.newskey, appskey: session.appskey, devaddr: session.devaddr }
    }
}

impl Session {
    pub fn derive_new<T: AsRef<[u8]>, F: CryptoFactory>(
        decrypt: &DecryptedJoinAcceptPayload<T, F>,
        devnonce: DevNonce,
        credentials: &NetworkCredentials,
    ) -> Self {
        Self::new(
            NewSKey(decrypt.derive_newskey(&devnonce, &credentials.appkey().0)),
            AppSKey(decrypt.derive_appskey(&devnonce, &credentials.appkey().0)),
            DevAddr::new([
                decrypt.dev_addr().as_ref()[0],
                decrypt.dev_addr().as_ref()[1],
                decrypt.dev_addr().as_ref()[2],
                decrypt.dev_addr().as_ref()[3],
            ])
            .unwrap(),
        )
    }

    pub fn new(newskey: NewSKey, appskey: AppSKey, devaddr: DevAddr<[u8; 4]>) -> Self {
        Self {
            newskey,
            appskey,
            devaddr,
            confirmed: false,
            fcnt_down: 0,
            fcnt_up: 0,
            uplink: uplink::Uplink::default(),
        }
    }

    pub fn devaddr(&self) -> &DevAddr<[u8; 4]> {
        &self.devaddr
    }
    pub fn appskey(&self) -> &AppSKey {
        &self.appskey
    }
    pub fn newskey(&self) -> &NewSKey {
        &self.newskey
    }

    pub fn get_session_keys(&self) -> Option<SessionKeys> {
        Some(SessionKeys { newskey: self.newskey, appskey: self.appskey, devaddr: self.devaddr })
    }
}

impl Session {
    pub(crate) fn handle_rx<C: CryptoFactory + Default, const N: usize, const D: usize>(
        &mut self,
        region: &mut region::Configuration,
        configuration: &mut super::Configuration,
        rx: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
        ignore_mac: bool,
    ) -> Response {
        if let Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) =
            lorawan_parse(rx.as_mut_for_read(), C::default())
        {
            if self.devaddr() == &encrypted_data.fhdr().dev_addr() {
                let fcnt = encrypted_data.fhdr().fcnt() as u32;
                let confirmed = encrypted_data.is_confirmed();
                if encrypted_data.validate_mic(&self.newskey().0, fcnt)
                    && (fcnt > self.fcnt_down || fcnt == 0)
                {
                    self.fcnt_down = fcnt;

                    // We can safely unwrap here because we already validated the MIC
                    let decrypted = encrypted_data
                        .decrypt(Some(&self.newskey().0), Some(&self.appskey().0), self.fcnt_down)
                        .unwrap();

                    if !ignore_mac {
                        // MAC commands may be in the FHDR or the FRMPayload
                        configuration.handle_downlink_macs(
                            region,
                            &mut self.uplink,
                            &mut decrypted.fhdr().fopts(),
                        );
                        if let Ok(FRMPayload::MACCommands(mac_cmds)) = decrypted.frm_payload() {
                            configuration.handle_downlink_macs(
                                region,
                                &mut self.uplink,
                                &mut mac_cmds.mac_commands(),
                            );
                        }
                    }

                    if confirmed {
                        self.uplink.set_downlink_confirmation();
                    }

                    return if self.fcnt_up == 0xFFFF_FFFF {
                        // if the FCnt is used up, the session has expired
                        Response::SessionExpired
                    } else {
                        // we can always increment fcnt_up when we receive a downlink
                        self.fcnt_up += 1;
                        if let (Some(fport), Ok(FRMPayload::Data(data))) =
                            (decrypted.f_port(), decrypted.frm_payload())
                        {
                            // heapless Vec from slice fails only if slice is too large.
                            // A data FRM payload will never exceed 256 bytes.
                            let data = Vec::from_slice(data).unwrap();
                            // TODO: propagate error type when heapless vec is full?
                            let _ = dl.push(Downlink { data, fport });
                        }
                        Response::DownlinkReceived(fcnt)
                    };
                }
            }
        }
        Response::NoUpdate
    }

    pub(crate) fn rx2_complete(&mut self) -> Response {
        // we only increment the fcnt_up if the uplink was not confirmed
        if !self.confirmed {
            if self.fcnt_up == 0xFFFF_FFFF {
                // if the FCnt is used up, the session has expired
                return Response::SessionExpired;
            } else {
                self.fcnt_up += 1;
            }
        }
        if self.confirmed {
            Response::NoAck
        } else {
            Response::RxComplete
        }
    }

    pub(crate) fn prepare_buffer<C: CryptoFactory + Default, const N: usize>(
        &mut self,
        data: &SendData,
        tx_buffer: &mut RadioBuffer<N>,
    ) -> FcntUp {
        tx_buffer.clear();
        let fcnt = self.fcnt_up;
        let mut phy: DataPayloadCreator<GenericArray<u8, U256>, C> = DataPayloadCreator::default();

        let mut fctrl = FCtrl(0x0, true);
        if self.uplink.confirms_downlink() {
            fctrl.set_ack();
            self.uplink.clear_downlink_confirmation();
        }

        self.confirmed = data.confirmed;

        phy.set_confirmed(data.confirmed)
            .set_fctrl(&fctrl)
            .set_f_port(data.fport)
            .set_dev_addr(self.devaddr)
            .set_fcnt(fcnt);

        let mut cmds = Vec::new();
        self.uplink.get_cmds(&mut cmds);
        let mut dyn_cmds: Vec<&dyn SerializableMacCommand, 8> = Vec::new();

        for cmd in &cmds {
            if let Err(_e) = dyn_cmds.push(cmd) {
                panic!("dyn_cmds too small compared to cmds")
            }
        }

        match phy.build(data.data, dyn_cmds.as_slice(), &self.newskey.0, &self.appskey.0) {
            Ok(packet) => {
                tx_buffer.clear();
                tx_buffer.extend_from_slice(packet).unwrap();
            }
            Err(e) => panic!("Error assembling packet! {} ", e),
        }
        fcnt
    }
}
