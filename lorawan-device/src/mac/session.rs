use super::{
    otaa::{DevNonce, NetworkCredentials},
    uplink, FcntUp, Response, SendData,
};
use crate::radio::RadioBuffer;
use crate::{region, AppSKey, Downlink, NwkSKey};
use heapless::Vec;
use lorawan::default_crypto::DefaultFactory;
use lorawan::maccommandcreator::{
    DevStatusAnsCreator, LinkADRAnsCreator, NewChannelAnsCreator, RXParamSetupAnsCreator,
    RXTimingSetupAnsCreator,
};
use lorawan::maccommands::{DownlinkMacCommand, MacCommandIterator};
use lorawan::{
    creator::DataPayloadCreator,
    parser::{parse as lorawan_parse, *},
};

#[cfg(feature = "certification")]
use super::DeviceEvent;

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
    // TODO: ADR handling
    #[cfg(feature = "certification")]
    /// Whether to force ADR bit for subsequent frames
    pub override_adr: bool,
    #[cfg(feature = "certification")]
    /// Whether to override confirmation bit for sent frames
    pub override_confirmed: Option<bool>,
    #[cfg(feature = "certification")]
    /// Applicative downlink frame counter
    pub rx_app_cnt: u16,
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
    pub fn derive_new<T: AsRef<[u8]>>(
        decrypt: &DecryptedJoinAcceptPayload<T>,
        devnonce: DevNonce,
        credentials: &NetworkCredentials,
    ) -> Self {
        Self::new(
            decrypt.derive_nwkskey(&devnonce, credentials.appkey(), &DefaultFactory),
            decrypt.derive_appskey(&devnonce, credentials.appkey(), &DefaultFactory),
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

            #[cfg(feature = "certification")]
            override_adr: false,
            #[cfg(feature = "certification")]
            override_confirmed: None,
            #[cfg(feature = "certification")]
            rx_app_cnt: 0,
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_rx<const N: usize, const D: usize>(
        &mut self,
        region: &mut region::Configuration,
        configuration: &mut super::Configuration,
        #[cfg(feature = "certification")] certification: &mut super::certification::Certification,
        #[cfg(feature = "multicast")] multicast: &mut super::multicast::Multicast,
        rx: &mut RadioBuffer<N>,
        dl: &mut Vec<Downlink, D>,
        ignore_mac: bool,
    ) -> Response {
        if let Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) =
            lorawan_parse(rx.as_mut_for_read())
        {
            // If ignore_mac is false, we're dealing with Class A downlink and
            // therefore can clear uplinks which need to be retained for acknowledgment
            if !ignore_mac {
                self.uplink.clear_mac_commands(false);
            }

            #[cfg(feature = "certification")]
            if let Some(port) = encrypted_data.f_port() {
                if port > 0 {
                    self.rx_app_cnt += 1;
                }
            }
            #[cfg(feature = "multicast")]
            if let Some(port) = encrypted_data.f_port() {
                if multicast.is_in_range(port) {
                    return multicast.handle_rx(dl, encrypted_data).into();
                }
            }
            let fcnt = encrypted_data.fhdr().fcnt() as u32;
            let confirmed = encrypted_data.is_confirmed();
            if encrypted_data.validate_mic(self.nwkskey().inner(), fcnt, &DefaultFactory)
                && (fcnt > self.fcnt_down || fcnt == 0)
            {
                self.fcnt_down = fcnt;
                // We can safely unwrap here because we already validated the MIC
                let decrypted = encrypted_data
                    .decrypt(
                        Some(self.nwkskey().inner()),
                        Some(self.appskey().inner()),
                        self.fcnt_down,
                        &DefaultFactory,
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
                        #[cfg(feature = "certification")]
                        if certification.fport(fport) {
                            use crate::mac::certification::Response::*;
                            match certification
                                .handle_message(data, self.fcnt_down.try_into().unwrap())
                            {
                                AdrBitChange(adr) => {
                                    self.override_adr = adr;
                                }
                                DutJoinReq => {
                                    return Response::DeviceHandler(DeviceEvent::ResetMac)
                                }
                                DutResetReq => {
                                    return Response::DeviceHandler(DeviceEvent::ResetDevice)
                                }
                                TxFramesCtrlReq(ftype) => {
                                    // None is a no-op, allowing network to trigger uplinks
                                    if ftype.is_some() {
                                        self.override_confirmed = ftype
                                    }
                                }
                                TxPeriodicityChange(periodicity) => {
                                    return Response::DeviceHandler(
                                        DeviceEvent::TxPeriodicityChange { periodicity },
                                    )
                                }
                                UplinkPrepared => return Response::UplinkPrepared,
                                NoUpdate => return Response::NoUpdate,
                            }
                        }
                        #[cfg(feature = "multicast")]
                        if multicast.is_remote_setup_port(fport) {
                            return multicast.handle_setup_message(data).into();
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

    pub(crate) fn prepare_buffer<const N: usize>(
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

        #[cfg(feature = "certification")]
        if self.override_adr {
            fctrl.set_adr()
        }

        self.confirmed = data.confirmed;
        #[cfg(feature = "certification")]
        if let Some(v) = self.override_confirmed {
            self.confirmed = v;
        }

        phy.set_confirmed(self.confirmed)
            .set_fctrl(&fctrl)
            .set_f_port(data.fport)
            .set_dev_addr(self.devaddr)
            .set_fcnt(fcnt);

        let crypto_factory = DefaultFactory;
        match phy.build(
            data.data,
            self.uplink.mac_commands(),
            &self.nwkskey,
            &self.appskey,
            &crypto_factory,
        ) {
            Ok(packet) => {
                self.uplink.clear_mac_commands(true);
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
        use DownlinkMacCommand::*;
        let mut channel_mask = region.channel_mask_get();
        let mut cmd_iter = cmds.into_iter().peekable();
        let mut num_adrreq = 0;
        while let Some(cmd) = cmd_iter.next() {
            match cmd {
                #[cfg(feature = "experimental")]
                DevStatusReq(..) => {
                    // TODO: Fill with proper values
                    // - Battery: (255 - unable to measure, 1..254 - battery level, 0 - external power source)
                    // - RadioStatus: (SNR: -32..31)
                    // For now we just return dummy values
                    let mut cmd = DevStatusAnsCreator::new();
                    let _ = cmd.set_battery(255).set_margin(0);
                    self.uplink.add_mac_command(cmd);
                }
                DlChannelReq(_payload) => {
                    if region.has_fixed_channel_plan() {
                        // Regions with fixed channel plan ignore this command
                        continue;
                    }
                    // TODO...
                }
                #[cfg(feature = "experimental")]
                LinkADRReq(payload) => {
                    // Contiguous LinkADRReq commands shall be processed in the
                    // order present in the downlink frame as a single atomic block
                    // command. For each command channel_mask is processed until
                    // reaching the last command of the block, when it's verified.
                    //
                    // DataRate, TxPower and NbTrans are processed only from the
                    // last LinkADRReq command.
                    //
                    // Number of LinkADRAns must match the number of LinkADRReq
                    // commands.
                    num_adrreq += 1;

                    // TODO: Validate that input is not RFU
                    let _ = region.channel_mask_update(
                        &mut channel_mask,
                        payload.redundancy().channel_mask_control(),
                        payload.channel_mask(),
                    );

                    // Check whether LinkADRReq commands continue...
                    if let Some(LinkADRReq(..)) = cmd_iter.peek() {
                        continue;
                    }

                    // ..if not, handle DataRate, TxPower and NbTrans and
                    // validate channel_mask.

                    // Handle DataRate
                    let dr = {
                        let rate = payload.data_rate();
                        // Use currently active rate in case requested rate is 15 (0xf)...
                        if rate == 0xf {
                            Some(configuration.data_rate)
                        } else {
                            region.check_data_rate(rate)
                        }
                    };
                    // Handle TxPower
                    let pw = {
                        let power = payload.tx_power();
                        // Use currently active power in case requested power is 15 (0xf)...
                        if power == 0xf {
                            Some(configuration.tx_power)
                        } else {
                            region.check_tx_power(power)
                        }
                    };

                    let cm_ack = region.channel_mask_validate(&channel_mask, dr);

                    if dr.is_some() && pw.is_some() && cm_ack {
                        // TODO: handle nbtrans
                        configuration.data_rate = dr.unwrap();
                        configuration.tx_power = pw.unwrap();
                        region.channel_mask_set(channel_mask.clone());
                    }

                    // Add matching number of LinkADRAns responses
                    for _ in 0..num_adrreq {
                        let mut cmd = LinkADRAnsCreator::new();
                        cmd.set_channel_mask_ack(cm_ack)
                            .set_data_rate_ack(dr.is_some())
                            .set_tx_power_ack(pw.is_some());
                        self.uplink.add_mac_command(cmd);
                    }
                    num_adrreq = 0;
                }
                NewChannelReq(payload) => {
                    if region.has_fixed_channel_plan() {
                        // Regions with fixed channel plan ignore this command
                        continue;
                    }
                    let (ack_f, ack_d) = region.handle_new_channel(
                        payload.channel_index(),
                        payload.frequency().value(),
                        payload.data_rate_range().ok(),
                    );

                    let mut cmd = NewChannelAnsCreator::new();
                    cmd.set_channel_frequency_ack(ack_f).set_data_rate_range_ack(ack_d);
                    self.uplink.add_mac_command(cmd);
                }
                #[cfg(feature = "experimental")]
                RXParamSetupReq(payload) => {
                    let freq = payload.frequency().value();
                    let freq_ack = region.frequency_valid(freq);

                    // TODO: Figure these out...
                    // let dl = payload.dl_settings();
                    // - rx1_dr_offset: dl.rx1_dr_offset()
                    // - rx2_data_rate = dl.rx2_data_rate());
                    if freq_ack {
                        configuration.rx2_frequency = Some(freq);
                    }

                    // RXParamSetupReq has its own acknowledgment mechanism, requiring
                    // RXParamSetupAns with all uplinks until a Class A downlink is received
                    // by the end-device.
                    let mut cmd = RXParamSetupAnsCreator::new();
                    cmd.set_rx1_data_rate_offset_ack(true)
                        .set_rx2_data_rate_ack(true)
                        .set_channel_ack(freq_ack);

                    self.uplink.add_mac_command(cmd);

                    // TODO: An end-device that expects to receive Class C
                    // downlink frames will send an uplink frame as soon
                    // as possible after receiving a valid RXParamSetupReq
                    // that modifies RX2 (Frequency or RX2DataRate fields).
                }
                RXTimingSetupReq(payload) => {
                    configuration.rx1_delay = super::del_to_delay_ms(payload.delay());
                    self.uplink.add_mac_command(RXTimingSetupAnsCreator::new());
                }
                _ => (),
            }
        }
    }
}
