use crate::{
    nb_device::Downlink, radio::RadioBuffer, region, FcntDown, FcntUp, SendData, SessionKeys,
};
use generic_array::{typenum::U256, GenericArray};
use heapless::Vec;
use lorawan::{
    self,
    creator::DataPayloadCreator,
    keys::CryptoFactory,
    maccommands::SerializableMacCommand,
    parser::{parse_with_factory as lorawan_parse, *},
};
pub(crate) mod types;
pub(crate) mod uplink;

#[derive(Debug, Default)]
pub struct Mac {
    uplink: uplink::Uplink,
    pub(crate) downlink: Option<Downlink>,
    fcnt_up: u32,
    fcnt_down: u32,
    confirmed: bool,
}

#[derive(Debug)]
pub enum Response {
    NoAck,
    SessionExpired,
    DownlinkReceived(FcntDown),
    ReadyToSend,
}

impl From<Response> for crate::Response {
    fn from(r: Response) -> Self {
        match r {
            Response::SessionExpired => crate::Response::SessionExpired,
            Response::DownlinkReceived(fcnt) => crate::Response::DownlinkReceived(fcnt),
            Response::NoAck => crate::Response::NoAck,
            Response::ReadyToSend => crate::Response::ReadyToSend,
        }
    }
}

impl Mac {
    pub(crate) fn handle_rx<C: CryptoFactory + Default>(
        &mut self,
        session: &SessionKeys,
        region: &mut region::Configuration,
        rx: &mut [u8],
    ) -> Option<Response> {
        if let Ok(PhyPayload::Data(DataPayload::Encrypted(encrypted_data))) =
            lorawan_parse(rx, C::default())
        {
            if session.devaddr() == &encrypted_data.fhdr().dev_addr() {
                let fcnt = encrypted_data.fhdr().fcnt() as u32;
                let confirmed = encrypted_data.is_confirmed();
                if encrypted_data.validate_mic(&session.newskey().0, fcnt)
                    && (fcnt > self.fcnt_down || fcnt == 0)
                {
                    self.fcnt_down = fcnt;
                    // increment the FcntUp since we have received
                    // downlink - only reason to not increment
                    // is if confirmed frame is sent and no
                    // confirmation (ie: downlink) occurs
                    self.fcnt_up += 1;

                    let mut copy = Vec::new();
                    copy.extend_from_slice(encrypted_data.as_bytes()).unwrap();

                    // there two unwraps that are sane in their own right
                    // * making a new EncryptedDataPayload with owned bytes will
                    //   always work when copy bytes from another
                    //   EncryptedPayload
                    // * the decrypt will always work when we have verified MIC
                    //   previously
                    let decrypted = EncryptedDataPayload::new_with_factory(copy, C::default())
                        .unwrap()
                        .decrypt(
                            Some(&session.newskey().0),
                            Some(&session.appskey().0),
                            self.fcnt_down,
                        )
                        .unwrap();

                    self.uplink.handle_downlink_macs(region, &mut decrypted.fhdr().fopts());
                    if confirmed {
                        self.uplink.set_downlink_confirmation();
                    }

                    if let Ok(FRMPayload::MACCommands(mac_cmds)) = decrypted.frm_payload() {
                        self.uplink.handle_downlink_macs(region, &mut mac_cmds.mac_commands());
                    }

                    self.downlink = Some(Downlink::Data(decrypted));

                    // check if FCnt is used up
                    return if self.fcnt_up == (0xFFFF + 1) {
                        // signal that the session is expired
                        // client must know to check for potential data
                        // (FCnt may be extracted when client checks)
                        Some(Response::SessionExpired)
                    } else {
                        Some(Response::DownlinkReceived(fcnt))
                    };
                }
            }
        }
        None
    }

    pub(crate) fn rx2_elapsed(&mut self) -> Response {
        if !self.confirmed {
            // if this was not a confirmed frame, we can still
            // increment the FCnt Up
            self.fcnt_up += 1;
        }

        if self.confirmed {
            Response::NoAck
        } else if self.fcnt_up == (0xFFFF + 1) {
            // signal that the session is expired
            // client must know to check for potential data
            Response::SessionExpired
        } else {
            Response::ReadyToSend
        }
    }

    #[allow(clippy::match_wild_err_arm)]
    pub(crate) fn prepare_buffer<C: CryptoFactory + Default, const N: usize>(
        &mut self,
        session: &SessionKeys,
        data: &SendData,
        tx_buffer: &mut RadioBuffer<N>,
    ) -> FcntUp {
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
            .set_dev_addr(*session.devaddr())
            .set_fcnt(fcnt);

        let mut cmds = Vec::new();
        self.uplink.get_cmds(&mut cmds);
        let mut dyn_cmds: Vec<&dyn SerializableMacCommand, 8> = Vec::new();

        for cmd in &cmds {
            if let Err(_e) = dyn_cmds.push(cmd) {
                panic!("dyn_cmds too small compared to cmds")
            }
        }

        match phy.build(data.data, dyn_cmds.as_slice(), &session.newskey().0, &session.appskey().0)
        {
            Ok(packet) => {
                tx_buffer.clear();
                tx_buffer.extend_from_slice(packet).unwrap();
            }
            Err(e) => panic!("Error assembling packet! {} ", e),
        }
        fcnt
    }

    pub fn fcnt_up(&self) -> u32 {
        self.fcnt_up
    }
}

pub fn del_to_delay_ms(del: u8) -> u32 {
    match del {
        2..=15 => del as u32 * 1000,
        _ => region::constants::RECEIVE_DELAY1,
    }
}
