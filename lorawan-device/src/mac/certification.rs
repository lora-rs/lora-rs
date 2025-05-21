use crate::mac;
use crate::radio::RadioBuffer;
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
    LinkCheckReq,
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
    pub(crate) fn handle_message(&mut self, data: &[u8], rx_app_cnt: u16) -> Response {
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
                LinkCheckReq(..) => return Response::LinkCheckReq,
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
                EchoIncPayloadReq(payload) => {
                    let mut buf: heapless::Vec<u8, 256> = heapless::Vec::new();
                    let mut ans = lorawan::certification::EchoIncPayloadAnsCreator::new();
                    ans.payload(payload.payload());
                    buf.extend_from_slice(ans.build()).unwrap();
                    self.pending_uplink = Some(buf);
                    return Response::UplinkPrepared;
                }
                RxAppCntReq(..) => {
                    let mut buf: heapless::Vec<u8, 256> = heapless::Vec::new();
                    let mut ans = lorawan::certification::RxAppCntAnsCreator::new();
                    ans.set_rx_app_cnt(rx_app_cnt);
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

    pub(crate) fn setup_send<const N: usize>(
        &mut self,
        mut state: &mut mac::State,
        buf: &mut RadioBuffer<N>,
    ) -> mac::Result<mac::FcntUp> {
        let send_data = mac::SendData {
            fport: CERTIFICATION_PORT,
            data: self.pending_uplink.as_ref().unwrap(),
            confirmed: false,
        };
        match &mut state {
            mac::State::Joined(ref mut session) => Ok(session.prepare_buffer::<N>(&send_data, buf)),
            mac::State::Otaa(_) => Err(mac::Error::NotJoined),
            mac::State::Unjoined => Err(mac::Error::NotJoined),
        }
    }
}
