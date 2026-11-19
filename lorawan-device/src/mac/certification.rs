use crate::region::constants::RP002_VERSION;
use lorawan::certification::parse_downlink_dut_commands;

/// Certification protocol uses `fport = 224`
pub(crate) const CERTIFICATION_PORT: u8 = 224;

/// LoRaWAN version reported without persistent identity: random DevNonce.
pub(crate) const LORAWAN_VERSION_1_0_2: [u8; 4] = [1, 0, 2, 0];
/// LoRaWAN version reported with persistent identity: monotonic DevNonce.
pub(crate) const LORAWAN_VERSION_1_0_4: [u8; 4] = [1, 0, 4, 0];

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub(crate) enum Response {
    NoUpdate,
    AdrBitChange(bool),
    DutJoinReq,
    DutResetReq,
    DutFPort224DisableReq,
    LinkCheckReq,
    DeviceTimeReq,
    TxFramesCtrlReq(Option<bool>),
    TxPeriodicityChange(Option<u16>),
    /// The command prepared an uplink answer that must be transmitted.
    CertificationAnswerSent,
}

pub(crate) struct Certification {
    pending_uplink: Option<heapless::Vec<u8, 256>>,
    /// Set for good by DutFPort224DisableReq or the application. FPort 224 downlinks
    /// are ordinary application data afterwards.
    port_disabled: bool,
    lorawan_version: [u8; 4],
}

impl Certification {
    pub fn new() -> Self {
        Self { pending_uplink: None, port_disabled: false, lorawan_version: LORAWAN_VERSION_1_0_2 }
    }

    pub(crate) fn set_port_disabled(&mut self, disabled: bool) {
        self.port_disabled = disabled;
    }

    pub(crate) fn set_lorawan_version(&mut self, version: [u8; 4]) {
        self.lorawan_version = version;
    }

    pub(crate) const fn port_disabled(&self) -> bool {
        self.port_disabled
    }
    pub(crate) fn handle_message(
        &mut self,
        data: &[u8],
        rx_app_cnt: u16,
        max_frm_payload_len: usize,
        firmware_version: [u8; 4],
    ) -> Response {
        use lorawan::certification::DownlinkDUTCommand::*;
        let messages = parse_downlink_dut_commands(data);
        for message in messages {
            let Ok(message) = message else {
                break;
            };
            match message {
                // Device layer
                DutJoinReq(..) => return Response::DutJoinReq,
                DutResetReq(..) => return Response::DutResetReq,
                DutFPort224DisableReq(..) => return Response::DutFPort224DisableReq,
                TxPeriodicityChangeReq(payload) => {
                    if let Ok(periodicity) = payload.periodicity() {
                        return Response::TxPeriodicityChange(periodicity);
                    }
                }
                // Responses with uplink
                LinkCheckReq(..) => return Response::LinkCheckReq,
                DeviceTimeReq(..) => return Response::DeviceTimeReq,
                DutVersionsReq(..) => {
                    let mut buf: heapless::Vec<u8, 256> = heapless::Vec::new();
                    let mut ans = lorawan::certification::DutVersionsAnsCreator::new();
                    let mut versions = [0u8; 12];
                    versions[..4].copy_from_slice(&firmware_version);
                    versions[4..8].copy_from_slice(&self.lorawan_version);
                    versions[8..].copy_from_slice(&RP002_VERSION);
                    ans.set_versions_raw(versions);
                    buf.extend_from_slice(ans.build()).unwrap();
                    self.pending_uplink = Some(buf);
                    return Response::CertificationAnswerSent;
                }
                EchoIncPayloadReq(payload) => {
                    let data = payload.payload();
                    let max_data_len = max_frm_payload_len
                        .saturating_sub(1)
                        .min(lorawan::certification::EchoIncPayloadAnsCreator::max_payload_len());
                    let data = &data[..data.len().min(max_data_len)];
                    if data.is_empty() {
                        return Response::NoUpdate;
                    }

                    let mut buf: heapless::Vec<u8, 256> = heapless::Vec::new();
                    let mut ans = lorawan::certification::EchoIncPayloadAnsCreator::new();
                    ans.payload(data);
                    buf.extend_from_slice(ans.build()).unwrap();
                    // The answer is a regular uplink. Clip it to what the
                    // current DR leaves for the FRMPayload.
                    self.pending_uplink = Some(buf);
                    return Response::CertificationAnswerSent;
                }
                RxAppCntReq(..) => {
                    let mut buf: heapless::Vec<u8, 256> = heapless::Vec::new();
                    let mut ans = lorawan::certification::RxAppCntAnsCreator::new();
                    ans.set_rx_app_cnt(rx_app_cnt);
                    buf.extend_from_slice(ans.build()).unwrap();
                    self.pending_uplink = Some(buf);
                    return Response::CertificationAnswerSent;
                }
                // MAC layer
                AdrBitChangeReq(payload) => {
                    if let Ok(adr) = payload.adr_enable() {
                        return Response::AdrBitChange(adr);
                    }
                }
                TxFramesCtrlReq(payload) => {
                    if let Ok(frametype) = payload.frame_type_override() {
                        return Response::TxFramesCtrlReq(frametype);
                    }
                }
                // Commands not implemented by the device are ignored.
                other => debug!("Ignoring unsupported certification command: {:?}", other),
            }
        }
        Response::NoUpdate
    }

    /// Whether a downlink on `fport` is a certification command for this device.
    pub(crate) const fn fport(&self, fport: u8) -> bool {
        !self.port_disabled && CERTIFICATION_PORT == fport
    }

    /// Consume the pending certification uplink, if any.
    pub(crate) fn take_pending_uplink(&mut self) -> Option<heapless::Vec<u8, 256>> {
        self.pending_uplink.take()
    }
}
