use super::{
    FcntUp, Response, SendData,
    otaa::{DevNonce, NetworkCredentials},
    uplink,
};
use crate::radio::RadioBuffer;
use crate::region::constants::{ADR_ACK_DELAY, ADR_ACK_LIMIT, MAX_FCNT_GAP};
use crate::{region, AppSKey, Downlink, NwkSKey};
use core::num::NonZeroU8;
use heapless::Vec;
use lorawan::creator::{DataFrame, Payload};
use lorawan::maccommandcreator::{
    DevStatusAnsCreator, DlChannelAnsCreator, LinkADRAnsCreator, NewChannelAnsCreator,
    RXParamSetupAnsCreator, RXTimingSetupAnsCreator,
};
use lorawan::maccommands::DownlinkMacCommand;
use lorawan::maccommands::{MacCommands, parse_downlink_mac_commands};
use lorawan::parser::{
    DataFrameType, DecryptedDataPayload, DecryptedJoinAcceptPayload, DevAddr, EncryptedDataPayload,
    FrmPayload,
};
use lorawan::{
    default_crypto::DefaultCrypto,
    packet_length::phy::{MHDR_LEN, MIC_LEN},
    types::DR,
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
    pub devaddr: DevAddr,
    pub fcnt_up: u32,
    /// Frame counter of the last accepted downlink, or `None` before the first
    /// downlink of the session. Only the low 16 bits of the counter are on the
    /// wire; the high 16 bits kept here are used to rebuild the full value.
    fcnt_down: Option<u32>,
    /// Uplinks since the last accepted downlink; used for ADRACKReq / ADR backoff.
    pub(crate) adr_ack_cnt: u32,
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
    pub devaddr: DevAddr,
}

impl From<Session> for SessionKeys {
    fn from(session: Session) -> Self {
        Self { nwkskey: session.nwkskey, appskey: session.appskey, devaddr: session.devaddr }
    }
}

impl Session {
    pub fn derive_new(
        decrypt: &DecryptedJoinAcceptPayload<'_>,
        devnonce: DevNonce,
        credentials: &NetworkCredentials,
    ) -> Self {
        Self::new(
            decrypt.derive_nwkskey(devnonce, &DefaultCrypto::new(credentials.appkey().inner())),
            decrypt.derive_appskey(devnonce, &DefaultCrypto::new(credentials.appkey().inner())),
            decrypt.dev_addr(),
        )
    }

    pub fn new(nwkskey: NwkSKey, appskey: AppSKey, devaddr: DevAddr) -> Self {
        Self {
            nwkskey,
            appskey,
            devaddr,
            confirmed: false,
            fcnt_down: None,
            fcnt_up: 0,
            adr_ack_cnt: 0,
            uplink: uplink::Uplink::default(),

            #[cfg(feature = "certification")]
            override_confirmed: None,
            #[cfg(feature = "certification")]
            rx_app_cnt: 0,
        }
    }

    pub fn devaddr(&self) -> &DevAddr {
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

    /// Frame counter of the last accepted downlink, or `None` before the first
    /// downlink of the session.
    pub fn fcnt_down(&self) -> Option<u32> {
        self.fcnt_down
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
        max_payload_len: u8,
        snr: i8,
        ignore_mac: bool,
    ) -> Response {
        let bytes = rx.as_mut_for_read();
        if let Ok(encrypted_data) = EncryptedDataPayload::parse(bytes) {
            {
                // Drop oversized packets which exceed the maximum allowed
                // transmission time defined by PHY layer.
                // Note that maximum defined size applies to MacPayload, but
                // DataPayload includes MHDR and MIC.
                let payload_len = encrypted_data.as_bytes().len();
                if payload_len > max_payload_len as usize + MHDR_LEN + MIC_LEN {
                    info!("Dropping oversized payload.");
                    return self.rx2_complete(configuration, region);
                }
            }

            // If ignore_mac is false, we're dealing with Class A downlink and
            // therefore can clear uplinks which need to be retained for acknowledgment
            if !ignore_mac {
                self.uplink.clear_mac_commands(false);
            }

            #[cfg(feature = "certification")]
            if let Some(port) = encrypted_data.f_port()
                && port > 0
            {
                self.rx_app_cnt += 1;
            }
            #[cfg(feature = "multicast")]
            if let Some(port) = encrypted_data.f_port()
                && multicast.is_in_range(port)
            {
                return multicast.handle_rx(dl, bytes).into();
            }
            let confirmed = encrypted_data.is_confirmed();
            let Some(fcnt) = next_fcnt_down(self.fcnt_down, encrypted_data.fhdr().fcnt()) else {
                return Response::NoUpdate;
            };
            let nwk_crypto = DefaultCrypto::new(self.nwkskey.inner());
            let app_crypto = DefaultCrypto::new(self.appskey.inner());
            if encrypted_data.validate_mic(&nwk_crypto, fcnt) {
                self.fcnt_down = Some(fcnt);
                // Any accepted downlink confirms connectivity for ADR.
                self.adr_ack_cnt = 0;
                // We can safely unwrap here because we already validated the MIC
                let decrypted = DecryptedDataPayload::decrypt_in_place(
                    bytes,
                    Some(&nwk_crypto),
                    Some(&app_crypto),
                    fcnt,
                )
                .unwrap();

                if !ignore_mac {
                    // MAC commands may be in the FHDR or the FRMPayload
                    self.handle_downlink_macs(
                        configuration,
                        region,
                        parse_downlink_mac_commands(decrypted.fhdr().f_opts()),
                        snr,
                    );
                    if let FrmPayload::MacCommands(mac_cmds) = decrypted.frm_payload() {
                        self.handle_downlink_macs(
                            configuration,
                            region,
                            parse_downlink_mac_commands(mac_cmds),
                            snr,
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
                    if let (Some(fport), FrmPayload::Data(data)) =
                        (decrypted.f_port(), decrypted.frm_payload())
                    {
                        #[cfg(feature = "certification")]
                        if certification.fport(fport) {
                            use crate::mac::certification::Response::*;
                            match certification.handle_message(data, fcnt as u16) {
                                AdrBitChange(adr) => {
                                    configuration.adr_enabled = adr;
                                }
                                DutJoinReq => {
                                    return Response::DeviceHandler(DeviceEvent::ResetMac);
                                }
                                DutResetReq => {
                                    return Response::DeviceHandler(DeviceEvent::ResetDevice);
                                }
                                LinkCheckReq => {
                                    return Response::LinkCheckReq;
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
                                    );
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

    pub(crate) fn rx2_complete(
        &mut self,
        configuration: &mut super::Configuration,
        region: &region::Configuration,
    ) -> Response {
        // Until we handle NbTrans, there is no case where we should not increment FCntUp.
        if self.fcnt_up == 0xFFFF_FFFF {
            // if the FCnt is used up, the session has expired
            return Response::SessionExpired;
        } else {
            self.fcnt_up += 1;
        }

        if configuration.adr_enabled {
            self.adr_ack_cnt = self.adr_ack_cnt.saturating_add(1);
            // After ADR_ACK_LIMIT + N*ADR_ACK_DELAY uplinks without a downlink,
            // step down the data rate to try to regain connectivity.
            if self.adr_ack_cnt >= (ADR_ACK_LIMIT + ADR_ACK_DELAY) as u32 {
                let past_limit = self.adr_ack_cnt - ADR_ACK_LIMIT as u32;
                if past_limit.is_multiple_of(ADR_ACK_DELAY as u32) {
                    if let Some(dr) = next_lower_datarate(region, configuration.data_rate) {
                        configuration.data_rate = dr;
                    }
                }
            }
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
        configuration: &super::Configuration,
        region: &region::Configuration,
    ) -> FcntUp {
        tx_buffer.clear();
        let fcnt = self.fcnt_up;
        let mut buf = [0u8; 256];

        let ack = self.uplink.confirms_downlink();
        if ack {
            self.uplink.clear_downlink_confirmation();
        }

        let adr = configuration.adr_enabled;
        // ADRACKReq asks the network for a downlink so ADR can keep working.
        // It is not set when already at the lowest usable data rate.
        let adr_ack_req = adr
            && self.adr_ack_cnt >= ADR_ACK_LIMIT as u32
            && next_lower_datarate(region, configuration.data_rate).is_some();

        self.confirmed = data.confirmed;
        #[cfg(feature = "certification")]
        if let Some(v) = self.override_confirmed {
            self.confirmed = v;
        }

        // FPort 0 sends the queued MAC commands as the FRMPayload (encrypted
        // with the NwkSKey) with FOpts left empty; the spec forbids
        // application data on port 0. Any other port piggybacks the queued
        // commands in FOpts.
        let (f_opts, payload) = match NonZeroU8::new(data.fport) {
            Some(f_port) => (self.uplink.mac_commands(), Payload::Data { f_port, data: data.data }),
            None => {
                if !data.data.is_empty() {
                    panic!("Error assembling packet! Data payload with fport 0 not allowed");
                }
                (&[][..], Payload::MacCommands(self.uplink.mac_commands()))
            }
        };
        let frame = DataFrame {
            frame_type: if self.confirmed {
                DataFrameType::ConfirmedUp
            } else {
                DataFrameType::UnconfirmedUp
            },
            dev_addr: self.devaddr,
            adr,
            adr_ack_req,
            ack,
            f_pending: false,
            fcnt,
            f_opts,
            payload,
        };
        let nwk_crypto = DefaultCrypto::new(self.nwkskey.inner());
        let app_crypto = DefaultCrypto::new(self.appskey.inner());
        match frame.build_into(&mut buf, &nwk_crypto, Some(&app_crypto)) {
            Ok(packet) => {
                tx_buffer.clear();
                tx_buffer.extend_from_slice(packet).unwrap();
            }
            Err(e) => panic!("Error assembling packet! {:?} ", e),
        }
        self.uplink.clear_mac_commands(true);
        fcnt
    }

    fn handle_downlink_macs(
        &mut self,
        configuration: &mut super::Configuration,
        region: &mut region::Configuration,
        cmds: MacCommands<'_, DownlinkMacCommand<'_>>,
        snr: i8,
    ) {
        use DownlinkMacCommand::*;
        let mut channel_mask = region.channel_mask_get();
        // The iterator is fused after the first malformed command, so this
        // processes the leading well-formed prefix of the stream.
        let mut cmd_iter = cmds.filter_map(Result::ok).peekable();
        let mut num_adrreq = 0;
        while let Some(cmd) = cmd_iter.next() {
            match cmd {
                DevStatusReq(..) => {
                    // TODO: Battery information should come from device/application
                    // Battery: (255 - unable to measure, 1..254 - battery level, 0 - external power source)
                    // For now we just return dummy value of "255"
                    let mut cmd = DevStatusAnsCreator::new();
                    let _ = cmd.set_battery(255).set_margin(snr);
                    self.uplink.add_mac_command(cmd);
                }
                DlChannelReq(payload) => {
                    if region.has_fixed_channel_plan() {
                        // Regions with fixed channel plan ignore this command
                        continue;
                    }
                    let (ack_f, ack_c) = region
                        .channel_dl_update(payload.channel_index(), payload.frequency().value());

                    let mut cmd = DlChannelAnsCreator::new();
                    cmd.set_channel_frequency_ack(ack_f).set_uplink_frequency_exists_ack(ack_c);
                    self.uplink.add_mac_command(cmd);
                }
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
                    let dr = match payload.data_rate() {
                        DR::_15 => Some(configuration.data_rate),
                        n => {
                            if region.get_datarate(n as u8).is_some() {
                                Some(n)
                            } else {
                                None
                            }
                        }
                    };
                    // Handle TxPower
                    let pw = match payload.tx_power() {
                        DR::_15 => Some(configuration.tx_power),
                        p => region.check_tx_power(p as u8),
                    };

                    let cm_ack = region.channel_mask_validate(&channel_mask, dr);
                    if cm_ack && let (Some(dr), Some(pw)) = (dr, pw) {
                        // TODO: handle nbtrans
                        configuration.data_rate = dr;
                        configuration.tx_power = pw;
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
                LinkCheckAns(..) => {
                    /* TODO: Payload contents are not consumed/handled
                     * by MAC layer, instead these might be useful to
                     * application layer.
                     * Therefore keep this as a placeholder until a proper
                     * device <-> mac integration has been implemented.
                     */
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
                RXParamSetupReq(payload) => {
                    let freq = payload.frequency().value();
                    let freq_ack = region.frequency_valid(freq);

                    let dl = payload.dl_settings();
                    let rx1_dr_offset = region.rx1_dr_offset_validate(dl.rx1_dr_offset());
                    let rx2_dr = match dl.rx2_data_rate() {
                        DR::_15 => Some(configuration.rx2_data_rate),
                        n => {
                            if region.get_datarate(n as u8).is_some() {
                                Some(Some(n))
                            } else {
                                None
                            }
                        }
                    };
                    if freq_ack && let (Some(rx2_dr), Some(rx1_dr_offset)) = (rx2_dr, rx1_dr_offset)
                    {
                        configuration.rx2_data_rate = rx2_dr;
                        configuration.rx2_frequency = Some(freq);
                        configuration.rx1_dr_offset = rx1_dr_offset;
                    }

                    let mut cmd = RXParamSetupAnsCreator::new();
                    cmd.set_rx1_data_rate_offset_ack(rx1_dr_offset.is_some())
                        .set_rx2_data_rate_ack(rx2_dr.is_some())
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

/// Next lower region-supported data rate, if any.
fn next_lower_datarate(region: &region::Configuration, current: DR) -> Option<DR> {
    let current = current as u8;
    if current == 0 {
        return None;
    }
    for candidate in (0..current).rev() {
        if region.get_datarate(candidate).is_some() {
            return Some(DR::from(candidate));
        }
    }
    None
}

/// Rebuild the full 32-bit downlink frame counter from the 16-bit value carried
/// on the wire and decide whether the frame is fresh.
///
/// Only the low 16 bits of the counter are transmitted (LoRaWAN 1.0.2
/// §4.3.1.5); the receiver keeps the high 16 bits in `last` and advances them
/// when the low half wraps. Returns the reconstructed counter to store, or
/// `None` when the frame must be dropped because its counter does not advance
/// past `last` or jumps further ahead than `MAX_FCNT_GAP` allows.
///
/// `last` is `None` until the first downlink of a session has been accepted, so
/// that first frame is taken at face value instead of being compared against an
/// initial counter.
fn next_fcnt_down(last: Option<u32>, wire: u16) -> Option<u32> {
    let Some(last) = last else {
        return Some(u32::from(wire));
    };
    let high = last & 0xFFFF_0000;
    let reconstructed = if wire >= last as u16 {
        high | u32::from(wire)
    } else {
        // The low half wrapped, so the frame belongs to the next 16-bit epoch.
        high.wrapping_add(0x1_0000) | u32::from(wire)
    };
    // Drop replays and counters that jump too far ahead. A stale frame from an
    // earlier counter reconstructs to a value far beyond `last`, so the gap
    // bound rejects it here before the MIC is even checked.
    match reconstructed.checked_sub(last) {
        Some(gap) if gap > 0 && gap <= MAX_FCNT_GAP as u32 => Some(reconstructed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::next_fcnt_down;
    use super::{SendData, Session};
    use crate::radio::RadioBuffer;
    use crate::{AppSKey, NwkSKey};
    use lorawan::default_crypto::DefaultCrypto;
    use lorawan::maccommandcreator::LinkADRAnsCreator;
    use lorawan::parser::{DecryptedDataPayload, DevAddr, FrmPayload};

    /// FPort 0 sends the queued MAC commands as the FRMPayload (encrypted
    /// with the NwkSKey), with FOpts left empty.
    #[test]
    fn fport_zero_sends_queued_mac_commands_in_frm_payload() {
        let nwkskey = NwkSKey::from([2; 16]);
        let appskey = AppSKey::from([1; 16]);
        let mut session = Session::new(nwkskey, appskey, DevAddr::from_value(1));

        let mut cmd = LinkADRAnsCreator::new();
        cmd.set_channel_mask_ack(true).set_data_rate_ack(true).set_tx_power_ack(true);
        let expected = cmd.build().to_vec();
        session.uplink.add_mac_command(cmd);

        let mut tx: RadioBuffer<256> = RadioBuffer::new();
        let mac = super::Mac::new(region::Configuration::new(region::Region::EU868), 14, 0);
        session.prepare_buffer::<256>(
            &SendData { data: &[], fport: 0, confirmed: false },
            &mut tx,
            &mac.configuration,
            &mac.region,
        );

        let bytes = tx.as_mut_for_read();
        let nwk_crypto = DefaultCrypto::new(nwkskey.inner());
        let decrypted =
            DecryptedDataPayload::decrypt_in_place(bytes, Some(&nwk_crypto), None, 0).unwrap();
        assert_eq!(decrypted.fhdr().f_opts(), &[] as &[u8]);
        assert_eq!(decrypted.f_port(), Some(0));
        assert_eq!(decrypted.frm_payload(), FrmPayload::MacCommands(&expected[..]));
    }

    #[test]
    fn first_downlink_taken_at_face_value() {
        // Before any downlink is seen, the wire value is accepted as-is even
        // when it is zero (the counter both ends start from).
        assert_eq!(next_fcnt_down(None, 0), Some(0));
        assert_eq!(next_fcnt_down(None, 7), Some(7));
    }

    #[test]
    fn increasing_counter_in_same_epoch() {
        assert_eq!(next_fcnt_down(Some(5), 6), Some(6));
        assert_eq!(next_fcnt_down(Some(100), 200), Some(200));
    }

    #[test]
    fn jump_beyond_max_fcnt_gap_is_dropped() {
        use super::MAX_FCNT_GAP;
        let last = 5;
        // A jump of exactly MAX_FCNT_GAP is still accepted.
        let at_limit = last + MAX_FCNT_GAP as u16;
        assert_eq!(next_fcnt_down(Some(last as u32), at_limit), Some(at_limit as u32));
        // One past the limit is rejected.
        assert_eq!(next_fcnt_down(Some(last as u32), at_limit + 1), None);
    }

    #[test]
    fn replayed_or_stale_counter_is_dropped() {
        // Exact replay of the last counter.
        assert_eq!(next_fcnt_down(Some(6), 6), None);
        // An older counter from the same epoch.
        assert_eq!(next_fcnt_down(Some(6), 5), None);
        // Once a downlink has been seen, a wire value of zero is stale like
        // any other: it neither resets the counter nor bypasses the check.
        assert_eq!(next_fcnt_down(Some(6), 0), None);
    }

    #[test]
    fn counter_is_reconstructed_past_16_bits() {
        // Wire wraps 0xFFFF -> 0x0000: the reconstructed value crosses into the
        // next epoch rather than folding back to zero.
        assert_eq!(next_fcnt_down(Some(0xFFFF), 0), Some(0x1_0000));
        // Further progress inside the high epoch.
        assert_eq!(next_fcnt_down(Some(0x1_0000), 1), Some(0x1_0001));
        // A frame far into the session reconstructs correctly instead of
        // capping at 0xFFFF.
        assert_eq!(next_fcnt_down(Some(0x0003_FFFE), 0xFFFF), Some(0x0003_FFFF));
        assert_eq!(next_fcnt_down(Some(0x0003_FFFF), 0), Some(0x0004_0000));
    }

    #[test]
    fn near_top_of_range_does_not_wrap_backwards() {
        // Reconstruction that would overflow the 32-bit counter is rejected
        // rather than wrapping to a smaller value.
        assert_eq!(next_fcnt_down(Some(0xFFFF_FFFE), 0), None);
    }
}
