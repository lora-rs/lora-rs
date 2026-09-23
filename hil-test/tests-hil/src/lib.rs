//! Network-side HIL fixture for lora-rs: a Semtech-UDP "network server" that
//! services OTAA joins from the bench DUT and emits typed events so host-side
//! tests can assert on the RF/crypto path.

pub mod relay;

use lora_modulation::{Bandwidth, SpreadingFactor};
use lorawan::default_crypto::{DefaultCrypto, DefaultNetworkCrypto};
use lorawan::keys::{AES128, AppSKey, Crypto, NetworkCrypto, NwkSKey};
use lorawan::parser::{
    DecryptedDataPayload, DecryptedJoinAcceptPayload, FrmPayload, PhyPayload,
    parse as parse_lorawan,
};
use semtech_udp::pull_resp::{self, PhyData, Time};
use semtech_udp::push_data::RxPk;
use semtech_udp::server_runtime::{Error, Event, UdpRuntime};
use semtech_udp::{CodingRate, DataRate, MacAddress, Modulation, tx_ack};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;

/// Test credentials baked into the HIL firmware.
pub const APP_KEY: AES128 = AES128([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
pub const DEV_ADDR: [u8; 4] = [0x48, 0x49, 0x4C, 0x01]; // "HIL\x01"
pub const NET_ID: [u8; 3] = [0x00, 0x00, 0x01];

/// US915: JoinAccept RX1 opens JOIN_ACCEPT_DELAY1 = 5 s after the uplink.
const JOIN_ACCEPT_DELAY1_US: u32 = 5_000_000;

/// US915 downlink channels; RX1 channel = uplink channel % 8.
const DOWNLINK_CHANNEL_MHZ: [f64; 8] = [923.3, 923.9, 924.5, 925.1, 925.7, 926.3, 926.9, 927.5];

/// What the harness should do with incoming JoinRequests.
#[derive(Clone, Debug, Default)]
pub enum JoinPolicy {
    /// Validate MIC and answer with a well-formed JoinAccept (subband-2 CFList).
    #[default]
    Accept,
    /// Stay silent: for tests asserting join failure on the device.
    Ignore,
    /// Answer with a JoinAccept whose MIC is corrupted: device must reject it.
    TamperMic,
}

/// RF parameters carried by the JoinAccept, settable per scenario to validate
/// that the device actually adopts them (lora-rs PR #461).
#[derive(Clone, Copy, Debug)]
pub struct JoinAcceptParams {
    /// DLSettings byte: RX1DROffset in bits 6:4, RX2 datarate in bits 3:0.
    pub dl_settings: u8,
    /// RECEIVE_DELAY1 in seconds; RX2 opens one second later.
    pub rx_delay: u8,
}

impl Default for JoinAcceptParams {
    fn default() -> Self {
        // US915 defaults: RX1DROffset 0, RX2 DR8, 1 s delay.
        Self { dl_settings: 0x08, rx_delay: 1 }
    }
}

/// Which RX window a post-join data downlink targets.
#[derive(Clone, Copy, Debug)]
pub enum RxWindow {
    Rx1,
    Rx2,
}

/// When set, the harness answers every MIC-valid data uplink with this
/// downlink, timed and datarated from its `JoinAcceptParams`. A device that
/// did not adopt those parameters listens in the wrong place and misses it.
#[derive(Clone, Debug)]
pub struct UplinkResponse {
    pub payload: Vec<u8>,
    pub fport: u8,
    pub window: RxWindow,
}

/// Typed events for test assertions. Every event also carries enough detail
/// to debug a failed run from the transcript.
#[derive(Debug)]
pub enum HarnessEvent {
    GatewayConnected {
        mac: MacAddress,
    },
    JoinRequestReceived {
        dev_nonce: [u8; 2],
        freq_hz: u32,
        rssi: i32,
    },
    /// A JoinRequest whose MIC failed validation (foreign or corrupt).
    JoinRequestBadMic,
    JoinAcceptSent {
        dev_addr: [u8; 4],
        join_nonce: [u8; 3],
        rx1_freq_mhz: f64,
        tmst: u32,
    },
    /// TX_ACK (or power-adjusted ack) from the gateway for our downlink.
    DownlinkAcked,
    DownlinkFailed {
        error: String,
    },
    HeartbeatDecrypted {
        fcnt: u32,
        fport: u8,
        payload: Vec<u8>,
    },
    /// Post-join data downlink handed to the gateway (see `UplinkResponse`).
    DataDownlinkScheduled {
        fcnt_down: u32,
        freq_mhz: f64,
        tmst: u32,
    },
    /// Data uplink for our DevAddr that failed MIC (stale session?).
    DataBadMic {
        fcnt: u32,
    },
    /// Traffic from a device that isn't ours (busy RF neighborhood).
    ForeignTraffic,
}

pub struct Session {
    pub dev_addr: [u8; 4],
    pub nwk_skey: NwkSKey,
    pub app_skey: AppSKey,
    pub fcnt_down: u32,
}

pub struct Harness {
    udp: UdpRuntime,
    pub policy: JoinPolicy,
    pub join_params: JoinAcceptParams,
    pub uplink_response: Option<UplinkResponse>,
    join_nonce_counter: u32,
    pub session: Option<Session>,
    events: mpsc::UnboundedSender<HarnessEvent>,
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// Map an uplink frequency (Hz) to its US915 uplink channel number.
pub fn uplink_channel(freq_hz: u32, bw: Bandwidth) -> Option<u32> {
    match bw {
        Bandwidth::_125KHz => {
            let off = freq_hz.checked_sub(902_300_000)?;
            (off % 200_000 == 0 && off / 200_000 < 64).then_some(off / 200_000)
        }
        Bandwidth::_500KHz => {
            let off = freq_hz.checked_sub(903_000_000)?;
            (off % 1_600_000 == 0 && off / 1_600_000 < 8).then_some(64 + off / 1_600_000)
        }
        _ => None,
    }
}

/// US915 uplink DR index for an uplink modulation.
pub fn uplink_dr(datarate: &DataRate) -> Option<u8> {
    match (datarate.bandwidth(), datarate.spreading_factor()) {
        (Bandwidth::_125KHz, SpreadingFactor::_10) => Some(0),
        (Bandwidth::_125KHz, SpreadingFactor::_9) => Some(1),
        (Bandwidth::_125KHz, SpreadingFactor::_8) => Some(2),
        (Bandwidth::_125KHz, SpreadingFactor::_7) => Some(3),
        (Bandwidth::_500KHz, SpreadingFactor::_8) => Some(4),
        _ => None,
    }
}

/// US915 downlink modulation for a downlink DR index (DR8-13, all 500 kHz).
pub fn downlink_datarate(dr: u8) -> Option<DataRate> {
    let sf = match dr {
        8 => SpreadingFactor::_12,
        9 => SpreadingFactor::_11,
        10 => SpreadingFactor::_10,
        11 => SpreadingFactor::_9,
        12 => SpreadingFactor::_8,
        13 => SpreadingFactor::_7,
        _ => return None,
    };
    Some(DataRate::new(sf, Bandwidth::_500KHz))
}

/// US915 RX1 datarate for an uplink datarate: DR = clamp(10 + up - offset, 8, 13).
pub fn rx1_datarate(uplink: &DataRate, rx1_dr_offset: u8) -> Option<DataRate> {
    let up = uplink_dr(uplink)?;
    downlink_datarate((10 + up).saturating_sub(rx1_dr_offset).clamp(8, 13))
}

/// Build the encrypted JoinAccept PHYPayload and the session keys it implies.
///
/// Hand-rolled rather than via JoinAcceptCreator: US915 needs a type-1
/// (channel mask) CFList to pin the device to the gateway's subband, and the
/// creator only supports type-0 frequency lists.
pub fn build_join_accept(
    join_nonce: [u8; 3],
    dev_nonce: &[u8],
    params: JoinAcceptParams,
    tamper_mic: bool,
) -> Result<(Vec<u8>, NwkSKey, AppSKey), String> {
    let mut buf = [0u8; 33];
    buf[0] = 0x20; // MHDR: JoinAccept
    buf[1..4].copy_from_slice(&join_nonce);
    buf[4..7].copy_from_slice(&NET_ID);
    buf[7..11].copy_from_slice(&DEV_ADDR);
    buf[11] = params.dl_settings;
    buf[12] = params.rx_delay;
    // CFList type 1: ChannelMask<9> enabling subband 2 (ch 8-15) and its
    // 500 kHz partner ch 65, matching the gateway's listen window.
    buf[13..22].copy_from_slice(&[0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]);
    buf[28] = 0x01; // CFList type

    let crypto = DefaultNetworkCrypto::new(&APP_KEY);
    let mic = crypto.calculate_mic(&[], &buf[..29]);
    buf[29..33].copy_from_slice(&mic);
    if tamper_mic {
        buf[29] ^= 0xFF;
    }

    // JoinAccept "encryption" is aes128_decrypt so the device can encrypt.
    crypto.decrypt_block(&mut buf[1..17]);
    crypto.decrypt_block(&mut buf[17..33]);
    let phy = buf.to_vec();

    // Round-trip our own payload to derive the session keys the device will compute.
    let device_crypto = DefaultCrypto::new(&APP_KEY);
    let mut round_trip = phy.clone();
    let decrypted = DecryptedJoinAcceptPayload::decrypt_in_place(&mut round_trip, &device_crypto)
        .map_err(|e| format!("{e:?}"))?;
    let dev_nonce: [u8; 2] = dev_nonce.try_into().map_err(|_| "bad DevNonce".to_string())?;
    let dev_nonce = lorawan::parser::DevNonce::from_wire_bytes(dev_nonce);
    let nwk_skey = decrypted.derive_nwkskey(dev_nonce, &device_crypto);
    let app_skey = decrypted.derive_appskey(dev_nonce, &device_crypto);
    Ok((phy, nwk_skey, app_skey))
}

impl Harness {
    /// Bind the Semtech-UDP server and return the harness plus its event stream.
    pub async fn bind(
        addr: SocketAddr,
    ) -> Result<(Self, mpsc::UnboundedReceiver<HarnessEvent>), Box<dyn std::error::Error>> {
        let udp = UdpRuntime::new(addr).await?;
        let (tx, rx) = mpsc::unbounded_channel();
        Ok((
            Self {
                udp,
                policy: JoinPolicy::default(),
                join_params: JoinAcceptParams::default(),
                uplink_response: None,
                join_nonce_counter: 0,
                session: None,
                events: tx,
            },
            rx,
        ))
    }

    fn emit(&self, ev: HarnessEvent) {
        println!("harness: {ev:?}");
        let _ = self.events.send(ev);
    }

    /// Service gateway traffic forever (or until the event receiver is dropped
    /// and a send fails silently; the loop itself never exits normally).
    pub async fn run(&mut self) {
        loop {
            match self.udp.recv().await {
                Event::NewClient((mac, addr)) => {
                    println!("gateway connected: {mac} from {addr}");
                    self.emit(HarnessEvent::GatewayConnected { mac });
                }
                Event::UpdateClient((mac, addr)) => {
                    println!("gateway readdressed: {mac} from {addr}");
                }
                Event::ClientDisconnected((mac, addr)) => {
                    println!("gateway disconnected: {mac} {addr}");
                }
                Event::PacketReceived(rxpk, mac) => {
                    println!(
                        "uplink from {mac}: {:.3} MHz {:?} rssi {} snr {} tmst {} [{}]",
                        rxpk.frequency(),
                        rxpk.datarate(),
                        rxpk.channel_rssi(),
                        rxpk.snr(),
                        rxpk.timestamp(),
                        hex(rxpk.data()),
                    );
                    self.handle_uplink(&rxpk, mac);
                }
                Event::StatReceived(stat, mac) => {
                    println!("stat from {mac}: {stat:?}");
                }
                Event::UnableToParseUdpFrame(err, buf) => {
                    println!("parse error: {err} ({} bytes)", buf.len());
                }
                Event::NoClientWithMac(_pkt, mac) => {
                    println!("no client with mac {mac:?}");
                }
            }
        }
    }

    fn handle_uplink(&mut self, rxpk: &RxPk, mac: MacAddress) {
        let freq_hz = (rxpk.frequency() * 1e6).round() as u32;
        let mut payload = rxpk.data().clone();
        let parsed = match parse_lorawan(payload.as_mut_slice()) {
            Ok(p) => p,
            Err(e) => {
                println!("  unparseable LoRaWAN payload ({e:?}): {}", hex(rxpk.data()));
                self.emit(HarnessEvent::ForeignTraffic);
                return;
            }
        };

        match parsed {
            PhyPayload::JoinRequest(join_request) => {
                if !join_request.validate_mic(&DefaultCrypto::new(&APP_KEY)) {
                    self.emit(HarnessEvent::JoinRequestBadMic);
                    return;
                }
                let dev_nonce = *join_request.dev_nonce().as_wire_bytes();
                self.emit(HarnessEvent::JoinRequestReceived {
                    dev_nonce,
                    freq_hz,
                    rssi: rxpk.channel_rssi(),
                });

                let tamper = match self.policy {
                    JoinPolicy::Accept => false,
                    JoinPolicy::TamperMic => true,
                    JoinPolicy::Ignore => return,
                };

                self.join_nonce_counter += 1;
                let jn = self.join_nonce_counter;
                let join_nonce =
                    [(jn & 0xFF) as u8, ((jn >> 8) & 0xFF) as u8, ((jn >> 16) & 0xFF) as u8];
                let (phy, nwk_skey, app_skey) =
                    match build_join_accept(join_nonce, &dev_nonce, self.join_params, tamper) {
                        Ok(r) => r,
                        Err(e) => {
                            println!("  failed to build JoinAccept: {e}");
                            return;
                        }
                    };

                let Some(channel) = uplink_channel(freq_hz, rxpk.datarate().bandwidth()) else {
                    println!("  uplink freq {freq_hz} Hz not a US915 channel, no downlink");
                    return;
                };
                // JoinAccept RX1 always uses offset 0: the accept itself is
                // what carries any nonstandard RX1DROffset to the device.
                let Some(datr) = rx1_datarate(&rxpk.datarate(), 0) else {
                    println!("  uplink datarate {:?} unsupported, no downlink", rxpk.datarate());
                    return;
                };
                let rx1_freq = DOWNLINK_CHANNEL_MHZ[(channel % 8) as usize];
                let tmst = rxpk.timestamp().wrapping_add(JOIN_ACCEPT_DELAY1_US);

                // Keys are the honest ones even when tampering: if the DUT
                // wrongly accepts a tampered frame, heartbeats will decrypt
                // and the test will see (and fail on) them.
                self.session =
                    Some(Session { dev_addr: DEV_ADDR, nwk_skey, app_skey, fcnt_down: 0 });
                self.emit(HarnessEvent::JoinAcceptSent {
                    dev_addr: DEV_ADDR,
                    join_nonce,
                    rx1_freq_mhz: rx1_freq,
                    tmst,
                });

                self.dispatch_downlink(phy, rx1_freq, tmst, datr, mac);
            }
            PhyPayload::Data(data) => {
                let Some(sess) = self.session.as_ref() else {
                    self.emit(HarnessEvent::ForeignTraffic);
                    return;
                };
                let fhdr = data.fhdr();
                if fhdr.dev_addr().as_wire_bytes() != &sess.dev_addr {
                    self.emit(HarnessEvent::ForeignTraffic);
                    return;
                }
                let fcnt = fhdr.fcnt() as u32;
                let nwk_crypto = DefaultCrypto::new(sess.nwk_skey.inner());
                let app_crypto = DefaultCrypto::new(sess.app_skey.inner());
                if !data.validate_mic(&nwk_crypto, fcnt) {
                    self.emit(HarnessEvent::DataBadMic { fcnt });
                    return;
                }
                let fport = data.f_port().unwrap_or(0);
                match DecryptedDataPayload::decrypt_in_place(
                    payload.as_mut_slice(),
                    Some(&nwk_crypto),
                    Some(&app_crypto),
                    fcnt,
                ) {
                    Ok(decrypted) => {
                        let payload = match decrypted.frm_payload() {
                            FrmPayload::Data(bytes) => bytes.to_vec(),
                            _ => Vec::new(),
                        };
                        self.emit(HarnessEvent::HeartbeatDecrypted { fcnt, fport, payload });
                        if let Some(resp) = self.uplink_response.clone() {
                            self.respond_to_uplink(&resp, rxpk, mac);
                        }
                    }
                    Err(e) => println!("  data uplink decrypt failed: {e:?}"),
                }
            }
            other => {
                println!("  unexpected uplink type: {other:?}");
                self.emit(HarnessEvent::ForeignTraffic);
            }
        }
    }

    /// Build and schedule the configured data downlink in reply to a
    /// MIC-valid uplink, using this harness's `JoinAcceptParams` for window
    /// timing and datarate. Increments the session's FCntDown.
    fn respond_to_uplink(&mut self, resp: &UplinkResponse, rxpk: &RxPk, mac: MacAddress) {
        let Some(sess) = self.session.as_mut() else { return };
        let fcnt_down = sess.fcnt_down;

        let payload = match core::num::NonZeroU8::new(resp.fport) {
            Some(f_port) => lorawan::creator::Payload::Data { f_port, data: &resp.payload },
            None => lorawan::creator::Payload::MacCommands(&resp.payload),
        };
        let frame = lorawan::creator::DataFrame {
            frame_type: lorawan::parser::DataFrameType::UnconfirmedDown,
            dev_addr: lorawan::parser::DevAddr::from_wire_bytes(DEV_ADDR),
            fcnt: fcnt_down,
            payload,
            ..Default::default()
        };
        let mut buf = [0u8; 64];
        let nwk_crypto = DefaultCrypto::new(sess.nwk_skey.inner());
        let app_crypto = DefaultCrypto::new(sess.app_skey.inner());
        let phy = match frame.build_into(&mut buf, &nwk_crypto, Some(&app_crypto)) {
            Ok(p) => p.to_vec(),
            Err(e) => {
                println!("  data downlink build failed: {e:?}");
                return;
            }
        };
        sess.fcnt_down += 1;

        let rx_delay_us = u32::from(self.join_params.rx_delay.max(1)) * 1_000_000;
        let (freq, tmst, datr) = match resp.window {
            RxWindow::Rx1 => {
                let freq_hz = (rxpk.frequency() * 1e6).round() as u32;
                let Some(channel) = uplink_channel(freq_hz, rxpk.datarate().bandwidth()) else {
                    println!("  uplink freq {freq_hz} Hz not a US915 channel, no downlink");
                    return;
                };
                let offset = (self.join_params.dl_settings >> 4) & 0x07;
                let Some(datr) = rx1_datarate(&rxpk.datarate(), offset) else {
                    println!("  uplink datarate {:?} unsupported, no downlink", rxpk.datarate());
                    return;
                };
                let freq = DOWNLINK_CHANNEL_MHZ[(channel % 8) as usize];
                (freq, rxpk.timestamp().wrapping_add(rx_delay_us), datr)
            }
            RxWindow::Rx2 => {
                let rx2_dr = self.join_params.dl_settings & 0x0F;
                let Some(datr) = downlink_datarate(rx2_dr) else {
                    println!("  RX2 DR{rx2_dr} unsupported, no downlink");
                    return;
                };
                (923.3, rxpk.timestamp().wrapping_add(rx_delay_us + 1_000_000), datr)
            }
        };

        self.emit(HarnessEvent::DataDownlinkScheduled { fcnt_down, freq_mhz: freq, tmst });
        self.dispatch_downlink(phy, freq, tmst, datr, mac);
    }

    /// Hand a PHYPayload to the gateway as a PULL_RESP and report the TX_ACK
    /// outcome as an event.
    fn dispatch_downlink(
        &mut self,
        phy: Vec<u8>,
        freq: f64,
        tmst: u32,
        datr: DataRate,
        mac: MacAddress,
    ) {
        let txpk = pull_resp::TxPk {
            time: Time::by_tmst(tmst),
            freq,
            rfch: 0,
            // The DUT antenna is damaged; shout.
            powe: 27,
            modu: Modulation::LORA,
            datr,
            codr: Some(CodingRate::_4_5),
            ipol: true,
            data: PhyData::new(phy),
            fdev: None,
            prea: None,
            ncrc: Some(true),
        };
        let prepared_send = self.udp.prepare_downlink(txpk, mac);
        let events = self.events.clone();
        tokio::spawn(async move {
            match prepared_send.dispatch(Some(Duration::from_secs(10))).await {
                Ok(_) => {
                    let _ = events.send(HarnessEvent::DownlinkAcked);
                }
                Err(Error::Ack(tx_ack::Error::AdjustedTransmitPower(_, _))) => {
                    let _ = events.send(HarnessEvent::DownlinkAcked);
                }
                Err(e) => {
                    let _ = events.send(HarnessEvent::DownlinkFailed { error: format!("{e:?}") });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorawan::parser::CfList;
    use lorawan::types::ChannelMask;

    #[test]
    fn join_accept_round_trips() {
        let dev_nonce = [0x3D, 0xA8];
        let (phy, _, _) =
            build_join_accept([1, 0, 0], &dev_nonce, JoinAcceptParams::default(), false).unwrap();
        assert_eq!(phy.len(), 33);
        let crypto = DefaultCrypto::new(&APP_KEY);
        let mut buf = phy.clone();
        let decrypted =
            DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(&mut buf, &crypto).unwrap();
        assert_eq!(decrypted.dev_addr().as_wire_bytes(), &DEV_ADDR);
        let Some(CfList::FixedChannel(mask)) = decrypted.c_f_list() else {
            panic!("expected type-1 CFList, got {:?}", decrypted.c_f_list());
        };
        let expected = ChannelMask::<9>::new_from_raw(&[0, 0xFF, 0, 0, 0, 0, 0, 0, 0x02]);
        for ch in 0..72 {
            assert_eq!(
                mask.is_enabled(ch).unwrap(),
                expected.is_enabled(ch).unwrap(),
                "channel {ch}"
            );
        }
    }

    #[test]
    fn tampered_mic_fails_validation() {
        let (mut phy, _, _) =
            build_join_accept([1, 0, 0], &[0, 1], JoinAcceptParams::default(), true).unwrap();
        let crypto = DefaultCrypto::new(&APP_KEY);
        let decrypted =
            DecryptedJoinAcceptPayload::decrypt_in_place(phy.as_mut_slice(), &crypto).unwrap();
        assert!(!decrypted.validate_mic(&crypto));
    }

    #[test]
    fn join_accept_carries_params() {
        let params = JoinAcceptParams { dl_settings: 0x2A, rx_delay: 3 };
        let (mut phy, _, _) = build_join_accept([1, 0, 0], &[0x3D, 0xA8], params, false).unwrap();
        let crypto = DefaultCrypto::new(&APP_KEY);
        let decrypted =
            DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(phy.as_mut_slice(), &crypto)
                .unwrap();
        let dl = decrypted.dl_settings();
        assert_eq!(dl.rx1_dr_offset(), 2);
        assert_eq!(dl.rx2_data_rate() as u8, 10);
        assert_eq!(decrypted.rx_delay(), 3);
    }

    #[test]
    fn rx1_datarate_applies_offset_with_clamp() {
        let up = DataRate::new(SpreadingFactor::_10, Bandwidth::_125KHz); // DR0
        let dl = |offset| rx1_datarate(&up, offset).unwrap();
        assert_eq!(dl(0), DataRate::new(SpreadingFactor::_10, Bandwidth::_500KHz)); // DR10
        assert_eq!(dl(2), DataRate::new(SpreadingFactor::_12, Bandwidth::_500KHz)); // DR8
        assert_eq!(dl(3), DataRate::new(SpreadingFactor::_12, Bandwidth::_500KHz)); // clamped
    }
}
