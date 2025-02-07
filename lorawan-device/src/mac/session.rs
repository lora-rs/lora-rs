use super::{
    otaa::{DevNonce, NetworkCredentials},
    uplink, FcntUp, Response, SendData,
};
use crate::radio::RadioBuffer;
use crate::{region, AppSKey, Downlink, NwkSKey};
use heapless::Vec;
use lorawan::keys::CryptoFactory;
use lorawan::maccommandcreator::{LinkADRAnsCreator, RXTimingSetupAnsCreator};
use lorawan::maccommands::{DownlinkMacCommand, MacCommandIterator};
use lorawan::{
    creator::DataPayloadCreator,
    parser::{parse_with_factory as lorawan_parse, *},
};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Session {
    pub uplink: uplink::Uplink,
    pub confirmed: bool,
    pub nwkskey: NwkSKey,
    pub appskey: AppSKey,
    pub devaddr: DevAddr<[u8; 4]>,
    pub fcnt_up: u32,
    pub fcnt_down: u32,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct SessionKeys {
    pub nwkskey: NwkSKey,
    pub appskey: AppSKey,
    pub devaddr: DevAddr<[u8; 4]>,
}

impl From<Session> for SessionKeys {
    fn from(session: Session) -> Self {
        Self { nwkskey: session.nwkskey, appskey: session.appskey, devaddr: session.devaddr }
    }
}

impl Session {
    pub fn derive_new<T: AsRef<[u8]>, F: CryptoFactory>(
        decrypt: &DecryptedJoinAcceptPayload<T, F>,
        devnonce: DevNonce,
        credentials: &NetworkCredentials,
    ) -> Self {
        Self::new(
            decrypt.derive_nwkskey(&devnonce, credentials.appkey()),
            decrypt.derive_appskey(&devnonce, credentials.appkey()),
            DevAddr::new([
                decrypt.dev_addr().as_ref()[0],
                decrypt.dev_addr().as_ref()[1],
                decrypt.dev_addr().as_ref()[2],
                decrypt.dev_addr().as_ref()[3],
            ])
            .unwrap(),
        )
    }

    pub fn new(nwkskey: NwkSKey, appskey: AppSKey, devaddr: DevAddr<[u8; 4]>) -> Self {
        Self {
            nwkskey,
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
    #[deprecated(since = "0.12.2", note = "Please use `self.nwkskey` instead")]
    pub fn newskey(&self) -> &NwkSKey {
        &self.nwkskey
    }

    pub fn nwkskey(&self) -> &NwkSKey {
        &self.nwkskey
    }

    pub fn get_session_keys(&self) -> Option<SessionKeys> {
        Some(SessionKeys { nwkskey: self.nwkskey, appskey: self.appskey, devaddr: self.devaddr })
    }
}

impl Session {
    pub(crate) fn handle_rx<C: CryptoFactory + Default, const N: usize, const D: usize>(
        &mut self,
        region: &mut region::Configuration,
        configuration: &mut super::Configuration,
        #[cfg(feature = "multicast")] multicast: &mut super::multicast::Multicast,
        rx: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
        ignore_mac: bool,
    ) -> Response {
        if let Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) =
            lorawan_parse(rx.as_mut_for_read(), C::default())
        {
            #[cfg(feature = "multicast")]
            if let Some(port) = encrypted_data.f_port() {
                if multicast.is_in_range(port) {
                    return multicast.handle_rx(dl, encrypted_data).into();
                }
            }
            let fcnt = encrypted_data.fhdr().fcnt() as u32;
            let confirmed = encrypted_data.is_confirmed();
            if encrypted_data.validate_mic(self.nwkskey().inner(), fcnt)
                && (fcnt > self.fcnt_down || fcnt == 0)
            {
                self.fcnt_down = fcnt;
                // We can safely unwrap here because we already validated the MIC
                let decrypted = encrypted_data
                    .decrypt(
                        Some(self.nwkskey().inner()),
                        Some(self.appskey().inner()),
                        self.fcnt_down,
                    )
                    .unwrap();

                if !ignore_mac {
                    // MAC commands may be in the FHDR or the FRMPayload
                    self.handle_downlink_macs(
                        configuration,
                        region,
                        MacCommandIterator::<DownlinkMacCommand<'_>>::new(decrypted.fhdr().data()),
                    );
                    if let FRMPayload::MACCommands(mac_cmds) = decrypted.frm_payload() {
                        self.handle_downlink_macs(
                            configuration,
                            region,
                            MacCommandIterator::<DownlinkMacCommand<'_>>::new(mac_cmds.data()),
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
                    if let (Some(fport), FRMPayload::Data(data)) =
                        (decrypted.f_port(), decrypted.frm_payload())
                    {
                        #[cfg(feature = "multicast")]
                        if multicast.is_remote_setup_port(fport) {
                            return multicast.handle_setup_message::<C>(data).into();
                        }

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
        Response::NoUpdate
    }

    pub(crate) fn rx2_complete(&mut self) -> Response {
        // Until we handle NbTrans, there is no case where we should not increment FCntUp.
        if self.fcnt_up == 0xFFFF_FFFF {
            // if the FCnt is used up, the session has expired
            return Response::SessionExpired;
        } else {
            self.fcnt_up += 1;
        }
        if self.confirmed {
            Response::NoAck
        } else {
            Response::RxComplete
        }
    }

    pub(crate) fn prepare_buffer<C: CryptoFactory + Default, const N: usize>(
        &mut self,
        data: &SendData<'_>,
        tx_buffer: &mut RadioBuffer<N>,
    ) -> FcntUp {
        tx_buffer.clear();
        let fcnt = self.fcnt_up;
        let mut buf = [0u8; 256];
        let mut phy = DataPayloadCreator::new(&mut buf).unwrap();

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

        let crypto_factory = C::default();
        match phy.build(
            data.data,
            self.uplink.mac_commands(),
            &self.nwkskey,
            &self.appskey,
            &crypto_factory,
        ) {
            Ok(packet) => {
                self.uplink.clear_mac_commands();
                tx_buffer.clear();
                tx_buffer.extend_from_slice(packet).unwrap();
            }
            Err(e) => panic!("Error assembling packet! {:?} ", e),
        }
        fcnt
    }

    fn handle_downlink_macs(
        &mut self,
        configuration: &mut super::Configuration,
        region: &mut region::Configuration,
        cmds: MacCommandIterator<'_, DownlinkMacCommand<'_>>,
    ) {
        for cmd in cmds {
            match cmd {
                DownlinkMacCommand::LinkADRReq(payload) => {
                    // TODO: Verify with region that these are OK and handle Tx Power adjustment
                    region.set_channel_mask(
                        payload.redundancy().channel_mask_control(),
                        payload.channel_mask(),
                    );
                    let mut cmd = LinkADRAnsCreator::new();
                    cmd.set_channel_mask_ack(true).set_data_rate_ack(true).set_tx_power_ack(true);
                    self.uplink.add_mac_command(cmd);
                }
                DownlinkMacCommand::RXTimingSetupReq(payload) => {
                    configuration.rx1_delay = super::del_to_delay_ms(payload.delay());
                    self.uplink.add_mac_command(RXTimingSetupAnsCreator::new());
                }
                _ => (),
            }
        }
    }
}
