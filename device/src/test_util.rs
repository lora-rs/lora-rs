use super::*;
use lorawan::maccommands::{ChannelMask, SerializableMacCommand};
use lorawan::parser::DataHeader;
use lorawan::{
    default_crypto::DefaultFactory,
    maccommandcreator::LinkADRReqCreator,
    maccommands::{LinkADRReqPayload, MacCommand},
    parser::{parse, DataPayload, PhyPayload},
};
use radio::{RfConfig, TxConfig};
use std::vec::Vec;

/// This module contains some functions for both async device and state machine driven devices
/// to operate unit tests.
///

pub struct Uplink {
    data: Vec<u8>,
    #[allow(unused)]
    tx_config: TxConfig,
}

impl Uplink {
    /// Creates a copy from a reference and ensures the packet is at least parseable.
    pub fn new(data_in: &[u8], tx_config: TxConfig) -> Result<Self, &'static str> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(data_in);
        let _parse = parse(data.as_mut_slice())?;
        Ok(Self { data, tx_config: tx_config })
    }

    pub fn get_payload(&mut self) -> PhyPayload<&mut [u8], DefaultFactory> {
        // unwrap since we verified parse in new
        parse(self.data.as_mut_slice()).unwrap()
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

/// Handle join request and pack a JoinAccept into RxBuffer
pub fn handle_join_request(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let PhyPayload::JoinRequest(join_request) = uplink.get_payload() {
            assert!(join_request.validate_mic(&AES128(get_key())));
            let mut buffer: [u8; 17] = [0; 17];
            let mut phy = lorawan::creator::JoinAcceptCreator::with_options(
                &mut buffer,
                DefaultFactory::default(),
            )
            .unwrap();
            let app_nonce_bytes = [1; 3];
            phy.set_app_nonce(&app_nonce_bytes);
            phy.set_net_id(&[1; 3]);
            phy.set_dev_addr(get_dev_addr());
            let finished = phy.build(&AES128(get_key())).unwrap();
            rx_buffer[..finished.len()].copy_from_slice(&finished);
            return finished.len();
        }
    }
    0
}

/// Handle an uplink and respond with two LinkAdrReq on Port 0
pub fn handle_data_uplink_with_link_adr_req(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let PhyPayload::Data(DataPayload::Encrypted(data)) = uplink.get_payload() {
            let fcnt = data.fhdr().fcnt() as u32;
            assert!(data.validate_mic(&AES128(get_key()), fcnt));
            let uplink =
                data.decrypt(Some(&AES128(get_key())), Some(&AES128(get_key())), fcnt).unwrap();
            assert_eq!(uplink.fhdr().fcnt(), 0);
            let mac_cmds = [link_adr_req_with_bank_ctrl(0b10), link_adr_req_with_bank_ctrl(0b100)];
            let mac_cmds = [
                // drop the CID byte when building the MAC Command (ie: [1..])
                MacCommand::LinkADRReq(LinkADRReqPayload::new(&mac_cmds[0].build()[1..]).unwrap()),
                MacCommand::LinkADRReq(LinkADRReqPayload::new(&mac_cmds[1].build()[1..]).unwrap()),
            ];
            let cmd: Vec<&dyn SerializableMacCommand> = vec![&mac_cmds[0], &mac_cmds[1]];

            let mut phy = lorawan::creator::DataPayloadCreator::with_options(
                rx_buffer,
                DefaultFactory::default(),
            )
            .unwrap();
            phy.set_confirmed(uplink.is_confirmed());
            phy.set_dev_addr(&[0; 4]);
            phy.set_uplink(false);
            let finished = phy.build(&[], &cmd, &AES128(get_key()), &AES128(get_key())).unwrap();
            return finished.len();
        }
    }
    0
}

fn link_adr_req_with_bank_ctrl(cm: u16) -> LinkADRReqCreator {
    // prepare a confirmed downlink
    let mut adr_req = LinkADRReqCreator::new();
    adr_req.set_data_rate(0).unwrap();
    adr_req.set_tx_power(0).unwrap();
    // this should give us a chmask ctrl value of 5 which allows us to turn banks on and off
    adr_req.set_redundancy(0x50);
    // the second bit is the only high bit, so only bank 2 should be enabled
    let mut tmp = [cm as u8, (cm >> 8) as u8];
    let cm = ChannelMask::new(&mut tmp).unwrap();
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
            assert!(data.validate_mic(&AES128(get_key()), fcnt));
            let uplink =
                data.decrypt(Some(&AES128(get_key())), Some(&AES128(get_key())), fcnt).unwrap();
            let fhdr = uplink.fhdr();
            let mac_cmds: Vec<MacCommand> = fhdr.fopts().collect();

            assert_eq!(mac_cmds.len(), 2);
            assert!(matches!(mac_cmds[0], MacCommand::LinkADRAns(_)));
            assert!(matches!(mac_cmds[1], MacCommand::LinkADRAns(_)));

            // Build the actual data payload with FPort 0 which allows MAC Commands in payload
            rx_buffer.iter_mut().for_each(|x| *x = 0);
            let mut phy = lorawan::creator::DataPayloadCreator::with_options(
                rx_buffer,
                DefaultFactory::default(),
            )
            .unwrap();
            phy.set_confirmed(uplink.is_confirmed());
            phy.set_dev_addr(&[0; 4]);
            phy.set_uplink(false);
            //phy.set_f_port(3);
            phy.set_fcnt(1);
            // zero out rx_buffer
            let finished = phy.build(&[], &[], &AES128(get_key()), &AES128(get_key())).unwrap();
            return finished.len();
        }
    }
    0
}
