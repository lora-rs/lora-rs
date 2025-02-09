use lorawan::certification::parse_downlink_certification_messages;

/// Certification protocol uses `fport = 224`
pub(crate) const CERTIFICATION_PORT: u8 = 224;

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub(crate) enum Response {
    NoUpdate,
    TxFramesCtrlReq(Option<bool>),
}

pub(crate) struct Certification {}

impl Certification {
    pub(crate) fn handle_message(&mut self, data: &[u8]) -> Response {
        use lorawan::certification::DownlinkDUTCommand::*;
        let messages = parse_downlink_certification_messages(data);
        for message in messages {
            match message {
                // TODO: Device layer
                DutResetReq(..) | DutJoinReq(..) | DutVersionsReq(..) => {}
                // TODO: MAC layer
                AdrBitChangeReq(..) | EchoPayloadReq(..) | TxPeriodicityChangeReq(..) => {}
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
