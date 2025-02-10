use lorawan::certification::parse_downlink_certification_messages;

/// Certification protocol uses `fport = 224`
pub(crate) const CERTIFICATION_PORT: u8 = 224;

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub(crate) enum Response {
    NoUpdate,
    AdrBitChange(bool),
    DutJoinReq,
    DutResetReq,
    TxFramesCtrlReq(Option<bool>),
    TxPeriodicityChange(Option<u16>),
    UplinkPrepared,
}

pub(crate) struct Certification {
    pending_uplink: Option<heapless::Vec<u8, 256>>,
}

impl Certification {
    pub fn new() -> Self {
        Self { pending_uplink: None }
    }
    pub(crate) fn handle_message(&mut self, data: &[u8]) -> Response {
        use lorawan::certification::DownlinkDUTCommand::*;
        let messages = parse_downlink_certification_messages(data);
        for message in messages {
            match message {
                // Device layer
                DutJoinReq(..) => return Response::DutJoinReq,
                DutResetReq(..) => return Response::DutResetReq,
                TxPeriodicityChangeReq(payload) => {
                    if let Ok(periodicity) = payload.periodicity() {
                        return Response::TxPeriodicityChange(periodicity);
                    }
                }
                // Responses with uplink
                DutVersionsReq(..) => {
                    let mut buf: heapless::Vec<u8, 256> = heapless::Vec::new();
                    let mut ans = lorawan::certification::DutVersionsAnsCreator::new();
                    ans.set_versions_raw([
                        // TODO: Pass it via session::configuration?
                        0, 0, 0, 1, 1, 0, 4, 0, // Lorawan version (1.0.4 \o/)
                        // region version, RP002-1.0.4 == 2.1.0.4
                        // TODO: define and import from crate::region::constants::REGION_VERSION?
                        2, 1, 0, 4,
                    ]);
                    buf.extend_from_slice(ans.build()).unwrap();
                    self.pending_uplink = Some(buf);
                    return Response::UplinkPrepared;
                }
                EchoPayloadReq(payload) => {
                    let mut buf: heapless::Vec<u8, 256> = heapless::Vec::new();
                    let mut ans = lorawan::certification::EchoPayloadAnsCreator::new();
                    ans.payload(payload.payload());
                    buf.extend_from_slice(ans.build()).unwrap();
                    self.pending_uplink = Some(buf);
                    return Response::UplinkPrepared;
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
            }
        }
        Response::NoUpdate
    }

    pub(crate) const fn fport(&self, fport: u8) -> bool {
        CERTIFICATION_PORT == fport
    }
}
