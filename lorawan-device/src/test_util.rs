use super::*;
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, JoinAccept, Payload};
use lorawan::default_crypto::DefaultFactory;
use lorawan::maccommandcreator::LinkADRReqCreator;
use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::maccommands::UplinkMacCommand;
use lorawan::parser::{
    self, DataFrameType, DecryptedDataPayload, DecryptedJoinAcceptPayload, DevAddr, JoinNonce,
    NetId, PhyPayload,
};
use lorawan::types::{ChannelMask, DLSettings};
use mac::Session;

use radio::{RfConfig, TxConfig};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    vec::Vec,
};

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
        let _parse = parser::parse(&data)?;
        Ok(Self { data, tx_config })
    }

    /// The raw frame bytes; mutable so handlers can decrypt in place.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

/// Test functions shared by async_device and no_async_device tests
pub fn get_key() -> [u8; 16] {
    [0; 16]
}

pub fn get_dev_addr() -> DevAddr {
    DevAddr::from_value(0)
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
        nwkskey: NwkSKey::from(get_key()),
    }
}

pub type RxTxHandler = fn(Option<Uplink>, RfConfig, &mut [u8]) -> usize;

static SESSION: LazyLock<Mutex<HashMap<usize, Session>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Handle join request and pack a JoinAccept into RxBuffer
pub fn handle_join_request<const I: usize>(
    uplink: Option<Uplink>,
    config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    handle_join_request_with_dl_settings::<I, 0>(uplink, config, rx_buffer)
}

/// Handle join request and pack a JoinAccept carrying the given DLSettings byte into RxBuffer
pub fn handle_join_request_with_dl_settings<const I: usize, const DL_SETTINGS: u8>(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let Ok(PhyPayload::JoinRequest(join_request)) = parser::parse(uplink.data_mut()) {
            let devnonce = join_request.dev_nonce();
            assert!(join_request.validate_mic(&get_key().into(), &DefaultFactory));
            let accept = JoinAccept {
                join_nonce: JoinNonce::from_wire_bytes([1; 3]),
                net_id: NetId::from_wire_bytes([1; 3]),
                dev_addr: get_dev_addr(),
                dl_settings: DLSettings::new(DL_SETTINGS),
                rx_delay: 0,
                c_f_list: None,
            };
            let finished =
                accept.build_into(rx_buffer, &get_key().into(), &DefaultFactory).unwrap();
            let len = finished.len();

            let mut copy = finished.to_vec();
            let decrypt = DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(
                copy.as_mut_slice(),
                &get_key().into(),
                &DefaultFactory,
            )
            .expect("Somehow unable to parse my own join accept?");
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
            len
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
        let bytes = uplink.data_mut();
        let (fcnt, confirmed) = match parser::parse(&*bytes) {
            Ok(PhyPayload::Data(data)) => {
                let fcnt = data.fhdr().fcnt() as u32;
                assert!(data.validate_mic(&get_key().into(), fcnt, &DefaultFactory));
                (fcnt, data.is_confirmed())
            }
            _ => panic!("Did not decode PhyPayload::Data!"),
        };
        let decrypted = DecryptedDataPayload::decrypt_in_place(
            bytes,
            Some(&get_key().into()),
            Some(&get_key().into()),
            fcnt,
            &DefaultFactory,
        )
        .unwrap();
        assert_eq!(decrypted.fhdr().fcnt(), FCNT_UP);

        let mut cmds: Vec<u8> = Vec::new();
        cmds.extend_from_slice(link_adr_req_with_bank_ctrl(0b10).build());
        cmds.extend_from_slice(link_adr_req_with_bank_ctrl(0b100).build());

        let frame = DataFrame {
            frame_type: if confirmed {
                DataFrameType::ConfirmedDown
            } else {
                DataFrameType::UnconfirmedDown
            },
            dev_addr: get_dev_addr(),
            fcnt: FCNT_DOWN,
            f_opts: &cmds,
            payload: Payload::Data { f_port: NonZeroU8::new(4).unwrap(), data: &[3, 2, 1] },
            ..Default::default()
        };
        let finished = frame
            .build_into(rx_buffer, &get_key().into(), Some(&get_key().into()), &DefaultFactory)
            .unwrap();
        finished.len()
    } else {
        panic!("No uplink passed to handle_data_uplink_with_link_adr_req");
    }
}

/// Handle join request and respond with a JoinAccept built with the wrong key, so its MIC is
/// invalid. Sets RxDelay and DLSettings so tests can verify a rejected accept changes nothing.
pub fn handle_join_request_bad_mic(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    if let Some(mut uplink) = uplink {
        if let Ok(PhyPayload::JoinRequest(join_request)) = parser::parse(uplink.data_mut()) {
            assert!(join_request.validate_mic(&get_key().into(), &DefaultFactory));
            let accept = JoinAccept {
                join_nonce: JoinNonce::from_wire_bytes([1; 3]),
                net_id: NetId::from_wire_bytes([1; 3]),
                dev_addr: get_dev_addr(),
                dl_settings: DLSettings::new(0x2A),
                rx_delay: 3,
                c_f_list: None,
            };
            let wrong_key = [1; 16];
            let finished =
                accept.build_into(rx_buffer, &wrong_key.into(), &DefaultFactory).unwrap();
            finished.len()
        } else {
            panic!("Did not parse join request from uplink");
        }
    } else {
        panic!("No uplink passed to handle_join_request_bad_mic");
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
        let bytes = uplink.data_mut();
        let (fcnt, confirmed) = match parser::parse(&*bytes) {
            Ok(PhyPayload::Data(data)) => {
                let fcnt = data.fhdr().fcnt() as u32;
                assert!(data.validate_mic(&get_key().into(), fcnt, &DefaultFactory));
                (fcnt, data.is_confirmed())
            }
            _ => panic!(
                "Unable to parse PhyPayload::Data from uplink in handle_data_uplink_with_link_adr_ans"
            ),
        };
        let decrypted = DecryptedDataPayload::decrypt_in_place(
            bytes,
            Some(&get_key().into()),
            Some(&get_key().into()),
            fcnt,
            &DefaultFactory,
        )
        .unwrap();
        let fhdr = decrypted.fhdr();
        let mac_cmds: Vec<UplinkMacCommand<'_>> =
            parse_uplink_mac_commands(fhdr.f_opts()).collect::<Result<_, _>>().unwrap();

        assert_eq!(mac_cmds.len(), 2);
        assert!(matches!(mac_cmds[0], UplinkMacCommand::LinkADRAns(_)));
        assert!(matches!(mac_cmds[1], UplinkMacCommand::LinkADRAns(_)));

        // Respond with an empty downlink frame (no FPort, no payload).
        let frame = DataFrame {
            frame_type: if confirmed {
                DataFrameType::ConfirmedDown
            } else {
                DataFrameType::UnconfirmedDown
            },
            dev_addr: get_dev_addr(),
            fcnt: 1,
            ..Default::default()
        };
        let finished = frame
            .build_into(rx_buffer, &get_key().into(), Some(&get_key().into()), &DefaultFactory)
            .unwrap();
        finished.len()
    } else {
        panic!("No uplink passed to handle_data_uplink_with_link_adr_ans")
    }
}
