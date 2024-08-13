use super::*;
use lorawan::maccommands::{
    ChannelMask, DownlinkMacCommand, MacCommandIterator, SerializableMacCommand, UplinkMacCommand,
};
use lorawan::parser::{self, DataHeader};
use lorawan::{
    default_crypto::DefaultFactory,
    maccommandcreator::LinkADRReqCreator,
    maccommands::LinkADRReqPayload,
    parser::{parse, DataPayload, JoinAcceptPayload, PhyPayload},
};
use mac::Session;

use parser::FCtrl;
use radio::{RfConfig, TxConfig};
use std::{collections::HashMap, sync::Mutex, vec::Vec};

/// This module contains some functions for both async device and state machine driven devices
/// to operate unit tests.
///

#[derive(Debug, Clone)]
pub struct Uplink {
    data: Vec<u8>,
    #[allow(unused)]
    tx_config: TxConfig,
}

impl Uplink {
    /// Creates a copy from a reference and ensures the packet is at least parseable.
    pub fn new(data_in: &[u8], tx_config: TxConfig) -> Result<Self, parser::Error> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(data_in);
        let _parse = parse(data.as_mut_slice())?;
        Ok(Self { data, tx_config })
    }

    pub fn get_payload(&mut self) -> PhyPayload<&mut [u8], DefaultFactory> {
        match parse(self.data.as_mut_slice()) {
            Ok(p) => p,
            Err(e) => panic!("Failed to parse payload: {:?}", e),
        }
    }
}

/// Test functions shared by async_device and no_async_device tests
pub fn get_key() -> [u8; 16] {
    [0; 16]
}

pub fn get_dev_addr() -> DevAddr<[u8; 4]> {
    DevAddr::from(0)
}
pub fn get_otaa_credentials() -> JoinMode {
    JoinMode::OTAA {
        deveui: DevEui::from([0; 8]),
        appeui: AppEui::from([0; 8]),
        appkey: AppKey::from(get_key()),
    }
}

pub fn get_abp_credentials() -> JoinMode {
    JoinMode::ABP {
        devaddr: get_dev_addr(),
        appskey: AppSKey::from(get_key()),
        newskey: NewSKey::from(get_key()),
    }
}

pub type RxTxHandler = fn(Option<Uplink>, RfConfig, &mut [u8]) -> usize;

lazy_static::lazy_static! {
    static ref SESSION: Mutex<HashMap<usize, Session>> = Mutex::new(HashMap::new());

}

/// Handle join request and pack a JoinAccept into RxBuffer
pub fn handle_join_request<const I: usize>(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let PhyPayload::JoinRequest(join_request) = uplink.get_payload() {
            let devnonce = join_request.dev_nonce().to_owned();
            assert!(join_request.validate_mic(&get_key().into()));
            let mut buffer: [u8; 17] = [0; 17];
            let mut phy =
                lorawan::creator::JoinAcceptCreator::with_options(&mut buffer, DefaultFactory)
                    .unwrap();
            let app_nonce_bytes = [1; 3];
            phy.set_app_nonce(&app_nonce_bytes);
            phy.set_net_id(&[1; 3]);
            phy.set_dev_addr(get_dev_addr());
            let finished = phy.build(&get_key().into()).unwrap();
            rx_buffer[..finished.len()].copy_from_slice(finished);

            let mut copy = rx_buffer[..finished.len()].to_vec();
            if let PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(encrypted)) =
                parse(copy.as_mut_slice()).unwrap()
            {
                let decrypt = encrypted.decrypt(&get_key().into());
                let session = Session::derive_new(
                    &decrypt,
                    devnonce,
                    &NetworkCredentials::new(
                        AppEui::from([0; 8]),
                        DevEui::from([0; 8]),
                        AppKey::from(get_key()),
                    ),
                );
                {
                    let mut session_map = SESSION.lock().unwrap();
                    session_map.insert(I, session);
                }
            } else {
                panic!("Somehow unable to parse my own join accept?")
            }
            finished.len()
        } else {
            panic!("Did not parse join request from uplink");
        }
    } else {
        panic!("No uplink passed to handle_join_request");
    }
}

/// Handle an uplink and respond with two LinkAdrReq on Port 0
pub fn handle_data_uplink_with_link_adr_req<const FCNT_UP: u16, const FCNT_DOWN: u32>(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let PhyPayload::Data(DataPayload::Encrypted(data)) = uplink.get_payload() {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_key().into(), fcnt));
            let uplink =
                data.decrypt(Some(&get_key().into()), Some(&get_key().into()), fcnt).unwrap();
            assert_eq!(uplink.fhdr().fcnt(), FCNT_UP);
            let mac_cmds = [link_adr_req_with_bank_ctrl(0b10), link_adr_req_with_bank_ctrl(0b100)];
            let mac_cmds = [
                // drop the CID byte when building the MAC Command (ie: [1..])
                DownlinkMacCommand::LinkADRReq(
                    LinkADRReqPayload::new(&mac_cmds[0].build()[1..]).unwrap(),
                ),
                DownlinkMacCommand::LinkADRReq(
                    LinkADRReqPayload::new(&mac_cmds[1].build()[1..]).unwrap(),
                ),
            ];
            let cmd: Vec<&dyn SerializableMacCommand> = vec![&mac_cmds[0], &mac_cmds[1]];
            let mut phy =
                lorawan::creator::DataPayloadCreator::with_options(rx_buffer, DefaultFactory)
                    .unwrap();
            phy.set_confirmed(uplink.is_confirmed());
            phy.set_f_port(4);
            phy.set_dev_addr(&[0; 4]);
            phy.set_uplink(false);
            phy.set_fcnt(FCNT_DOWN);
            let finished =
                phy.build(&[3, 2, 1], &cmd, &get_key().into(), &get_key().into()).unwrap();
            finished.len()
        } else {
            panic!("Did not decode PhyPayload::Data!");
        }
    } else {
        panic!("No uplink passed to handle_data_uplink_with_link_adr_req");
    }
}

/// Handle an uplink and respond with two LinkAdrReq on Port 0
pub fn handle_class_c_uplink_after_join(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let PhyPayload::Data(DataPayload::Encrypted(data)) = uplink.get_payload() {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_key().into(), fcnt));
            let uplink =
                data.decrypt(Some(&get_key().into()), Some(&get_key().into()), fcnt).unwrap();
            assert_eq!(uplink.fhdr().fcnt(), 0);
            let mut phy =
                lorawan::creator::DataPayloadCreator::with_options(rx_buffer, DefaultFactory)
                    .unwrap();
            let mut fctrl = FCtrl::new(0, false);
            fctrl.set_ack();
            phy.set_confirmed(false);
            phy.set_dev_addr(&[0; 4]);
            phy.set_uplink(false);
            phy.set_fctrl(&fctrl);
            // set ack bit
            let finished = phy.build(&[], &[], &get_key().into(), &get_key().into()).unwrap();
            finished.len()
        } else {
            panic!("Did not decode PhyPayload::Data!");
        }
    } else {
        panic!("No uplink passed to handle_data_uplink_with_link_adr_req");
    }
}

fn link_adr_req_with_bank_ctrl(cm: u16) -> LinkADRReqCreator {
    // prepare a confirmed downlink
    let mut adr_req = LinkADRReqCreator::new();
    adr_req.set_data_rate(0).unwrap();
    adr_req.set_tx_power(0).unwrap();
    // this should give us a chmask ctrl value of 5 which allows us to turn banks on and off
    adr_req.set_redundancy(0x50);
    // the second bit is the only high bit, so only bank 2 should be enabled
    let tmp = [cm as u8, (cm >> 8) as u8];
    let cm = ChannelMask::new(&tmp).unwrap();
    adr_req.set_channel_mask(cm);
    adr_req
}

/// Looks for LinkAdrAns
pub fn handle_data_uplink_with_link_adr_ans(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let PhyPayload::Data(DataPayload::Encrypted(data)) = uplink.get_payload() {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&get_key().into(), fcnt));
            let uplink =
                data.decrypt(Some(&get_key().into()), Some(&get_key().into()), fcnt).unwrap();
            let fhdr = uplink.fhdr();
            let mac_cmds: Vec<UplinkMacCommand<'_>> =
                MacCommandIterator::<UplinkMacCommand<'_>>::new(fhdr.data()).collect();

            assert_eq!(mac_cmds.len(), 2);
            assert!(matches!(mac_cmds[0], UplinkMacCommand::LinkADRAns(_)));
            assert!(matches!(mac_cmds[1], UplinkMacCommand::LinkADRAns(_)));

            // Build the actual data payload with FPort 0 which allows MAC Commands in payload
            rx_buffer.iter_mut().for_each(|x| *x = 0);
            let mut phy =
                lorawan::creator::DataPayloadCreator::with_options(rx_buffer, DefaultFactory)
                    .unwrap();
            phy.set_confirmed(uplink.is_confirmed());
            phy.set_dev_addr(&[0; 4]);
            phy.set_uplink(false);
            //phy.set_f_port(3);
            phy.set_fcnt(1);
            // zero out rx_buffer
            let finished = phy.build(&[], &[], &get_key().into(), &get_key().into()).unwrap();
            finished.len()
        } else {
            panic!("Unable to parse PhyPayload::Data from uplink in handle_data_uplink_with_link_adr_ans")
        }
    } else {
        panic!("No uplink passed to handle_data_uplink_with_link_adr_ans")
    }
}

pub fn class_c_downlink<const FCNT_DOWN: u32>(
    _uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let mut phy =
        lorawan::creator::DataPayloadCreator::with_options(rx_buffer, DefaultFactory).unwrap();
    phy.set_f_port(3);
    phy.set_dev_addr(&[0; 4]);
    phy.set_uplink(false);
    phy.set_fcnt(FCNT_DOWN);
    let finished = phy.build(&[1, 2, 3], &[], &get_key().into(), &get_key().into()).unwrap();
    finished.len()
}
