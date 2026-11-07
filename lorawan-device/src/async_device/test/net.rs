//! A network-side test harness ("TCL") for driving the device.
//!
//! `Net` wraps the radio channel, the timer channel and the device under
//! test, like a network and its radio would. `exchange` runs one uplink,
//! `join` one OTAA join and (class C) `listen` one `rxc_listen`.
//!
//! Class C devices work too: their RXC listens select the radio against
//! the timer, so a timer fire with no radio event queued resolves on its
//! own. A downlink can be delivered into those listens with
//! `ExchangeAction::rxc_after`.

use super::{
    Device,
    radio::RadioChannel,
    timer::{TimerChannel, TimerKind},
};
use crate::async_device::{Error, JoinResponse, SendResponse, radio::RxConfig};
use crate::test_util::{Uplink, get_dev_addr, get_key, get_network_crypto, get_otaa_credentials};
use crate::{AppSKey, NwkSKey};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, JoinAccept, Payload};
use lorawan::default_crypto::{DefaultCrypto, DefaultNetworkCrypto};
use lorawan::parser::{CfList, DataFrameType, JoinNonce, NetId};
use lorawan::types::DLSettings;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

#[cfg(feature = "class-c")]
use crate::async_device::ListenResponse;

/// How long the harness waits for the device to make progress before it
/// declares the exchange stuck.
const STUCK: Duration = Duration::from_secs(5);

pub struct Net {
    radio: RadioChannel,
    timer: TimerChannel,
    device: Option<Device>,
    /// FCntDown of the network's next downlink, assigned by network.
    fcnt_down: u32,
    /// The session keys the network builds its frames with; follow the
    /// device's session (an OTAA join derives new ones).
    keys: SessionKeys,
}

/// The network's copy of the session keys.
struct SessionKeys {
    nwk: NwkSKey,
    app: AppSKey,
}

impl SessionKeys {
    fn of(device: &Device) -> Self {
        match device.mac.get_session() {
            Some(session) => Self { nwk: *session.nwkskey(), app: *session.appskey() },
            // No session yet: the one a join creates is not known in advance.
            None => Self { nwk: NwkSKey::from(get_key()), app: AppSKey::from(get_key()) },
        }
    }
}

impl Net {
    pub async fn new(radio: RadioChannel, timer: TimerChannel, device: Device) -> Self {
        // Uplinks the device transmitted before the network took over (eg
        // the one that enabled class C) belong to no exchange.
        radio.take_uplinks().await;
        radio.take_window_configs().await;
        // Continue the session's downlink counter: a repeated FCntDown is a
        // replay and the device drops the frame.
        let fcnt_down = device.mac.get_session().and_then(|s| s.fcnt_down()).map_or(0, |f| f + 1);
        let keys = SessionKeys::of(&device);
        Self { radio, timer, device: Some(device), fcnt_down, keys }
    }

    /// The device under test.
    pub fn device(&mut self) -> &mut Device {
        self.device.as_mut().expect("device not in use")
    }

    /// Run one uplink: the device sends `data` on `fport` and the network
    /// acts per `action`.
    /// Windows without a scripted downlink time out, and NbTrans
    /// retransmissions (and certification answers, which are regular
    /// uplinks) run to completion.
    /// Returns response and the uplinks observed on the air in correct order.
    pub async fn exchange(
        &mut self,
        data: &[u8],
        fport: u8,
        confirmed: bool,
        action: ExchangeAction,
    ) -> ExchangeResult {
        let data = data.to_vec(); // the spawned task owns the payload
        self.drive(
            move |mut device| async move {
                let response = device.send(&data, fport, confirmed).await;
                (device, response)
            },
            action,
        )
        .await
    }

    /// Run one OTAA join with the test credentials; the network acts per
    /// `action` (a `Pkt::join_accept` in some window, or nothing).
    pub async fn join(&mut self, action: ExchangeAction) -> JoinResult {
        let r = self
            .drive(
                |mut device| async move {
                    let response = device.join(&get_otaa_credentials()).await;
                    (device, response)
                },
                action,
            )
            .await;
        if matches!(r.response, Ok(JoinResponse::JoinSuccess)) {
            // A new session: fresh keys, and the downlink counter starts over.
            self.keys = SessionKeys::of(self.device());
            self.fcnt_down = 0;
        }
        r
    }

    /// Run one `rxc_listen`: the network delivers `frames` one by one into
    /// the RXC listen, waiting for the device to consume each. The listen
    /// must end on the last frame; uplinks the device transmits in answer
    /// to the frames are collected.
    #[cfg(feature = "class-c")]
    pub async fn listen(&mut self, frames: Vec<Pkt>) -> ListenResult {
        let device = self.device.take().expect("device not in use");
        let mut task = tokio::spawn(async move {
            let mut device = device;
            let response = device.rxc_listen().await;
            (device, response)
        });
        let mut fcnt_down = None;
        let mut frames = frames.into_iter();
        let (device, response) = loop {
            let Some(pkt) = frames.next() else {
                break tokio::time::timeout(STUCK, &mut task)
                    .await
                    .expect("listen did not end on the last frame")
                    .expect("device task panicked");
            };
            self.deliver(pkt, &mut fcnt_down).await;
            if let Some(outcome) = self.settle(&mut task).await {
                let undelivered = frames.len();
                assert_eq!(undelivered, 0, "listen ended with {undelivered} frames undelivered");
                break outcome;
            }
        };
        self.device = Some(device);
        Outcome {
            response,
            uplinks: self.radio.take_uplinks().await,
            fcnt_down,
            window_configs: Vec::new(),
        }
    }

    /// Run `op` on the device and drive the radio per `action` until it
    /// returns.
    async fn drive<R, F>(
        &mut self,
        op: impl FnOnce(Device) -> F,
        mut action: ExchangeAction,
    ) -> Outcome<R>
    where
        F: Future<Output = (Device, R)> + Send + 'static,
        R: Send + 'static,
    {
        let mut last = self.timer.get_armed_count().await;
        let device = self.device.take().expect("device not in use");
        #[cfg(feature = "class-c")]
        let class_c = device.class_c;
        #[cfg(not(feature = "class-c"))]
        let class_c = false;
        assert!(action.rxc.is_empty() || class_c, "rxc_after needs a class C device");
        let mut task = tokio::spawn(op(device));

        // Drive the radio:
        // a) wait for the device to arm a timer (a rx window or a retransmission wait)
        // b) class C: deliver a downlink into the RXC listen before the fire
        // c) fire timer
        // d) once window is about to open:
        // - deliver the scripted downlink or...
        // - timeout
        // A retransmission wait is not followed by a radio read, so no
        // radio event is sent for it.
        // Repeat until `op` returns.
        let mut window = 0;
        let mut fcnt_down = None;
        let (device, response) = loop {
            let step = tokio::time::timeout(STUCK, async {
                tokio::select! {
                    joined = &mut task => Step::Done(Box::new(joined.expect("device task panicked"))),
                    armed = self.timer.wait_armed(last + 1) => Step::Armed(armed),
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!("exchange stuck: waiting for a timer arm or the device to finish; window={window} last={last}")
            });
            match step {
                Step::Done(outcome) => break *outcome,
                Step::Armed(armed) => last = armed,
            }
            if let Some(pkt) = action.take_rxc(window) {
                self.deliver(pkt, &mut fcnt_down).await;
                // The device either ends the exchange on the downlink or
                // keeps waiting on the timer it armed.
                if let Some(outcome) = self.settle(&mut task).await {
                    break outcome;
                }
            }
            // Read the kind before firing:
            // firing wakes the device, this can arm the next timer and
            // overwrite the kind.
            let kind = self.timer.last_timer_kind();
            self.timer.fire_most_recent().await;
            if kind == TimerKind::Delay {
                continue;
            }
            window += 1;

            match action.take(window) {
                Some(Rx::Frame(pkt)) => self.deliver(pkt, &mut fcnt_down).await,
                Some(Rx::PhyError(e)) => self.radio.handle_phy_error(e).await,
                None => self.radio.handle_timeout().await,
            }
        };

        if let Some(w) = action.windows.iter().map(|(w, _)| *w).min() {
            panic!(
                "scripted downlink for window {w} was never delivered: the exchange ended after {window} windows"
            );
        }
        if let Some(w) = action.rxc.iter().map(|(w, _)| *w).min() {
            panic!(
                "scripted RXC downlink after window {w} was never delivered: the exchange ended after {window} windows"
            );
        }
        let uplinks = self.radio.take_uplinks().await;
        let window_configs = self.radio.take_window_configs().await;
        assert_eq!(window_configs.len(), window, "one RX window configuration per window");
        self.device = Some(device);
        Outcome { response, uplinks, fcnt_down, window_configs }
    }

    /// Build `pkt` with the network's counter and hand it to the radio.
    async fn deliver(&mut self, pkt: Pkt, fcnt_down: &mut Option<u32>) {
        let fcnt = self.fcnt_down;
        let mut buf = [0u8; 256];
        let len = pkt.build(&mut buf, fcnt, &self.keys);
        self.radio.handle_frame(buf[..len].to_vec()).await;
        if pkt.consumes_fcnt_down() {
            self.fcnt_down += 1;
            *fcnt_down = Some(fcnt);
        }
    }

    /// After a frame delivered outside an RX window: wait for the device to
    /// consume it and either finish or carry on waiting.
    async fn settle<T>(&mut self, task: &mut JoinHandle<T>) -> Option<T> {
        let consumed = async {
            self.radio.wait_consumed().await;
            // Give the device the same head start the fixture gives every
            // radio event, so an exchange it ends is seen as ended.
            tokio::time::sleep(Duration::from_millis(5)).await;
        };
        tokio::select! {
            joined = task => Some(joined.expect("device task panicked")),
            _ = consumed => None,
        }
    }
}

/// One step of the exchange loop.
enum Step<R> {
    /// The device operation returned.
    Done(Box<(Device, R)>),
    /// The device armed a timer; the armed count so far.
    Armed(usize),
}

/// A frame builder for a downlink the network builds itself, given the
/// FCntDown the network assigns.
type FrameBuilder = Arc<dyn Fn(&mut [u8], u32) -> usize + Send + Sync>;

/// A downlink to be delivered by the network.
/// A data frame carries no FCntDown: the network assigns it from its own
/// counter when the frame is built.
#[derive(Clone)]
pub struct Pkt(PktKind);

#[derive(Clone)]
enum PktKind {
    Data {
        frame_type: DataFrameType,
        ack: bool,
        /// MAC commands piggybacked in FOpts.
        fopts: Vec<u8>,
        /// (fport, payload); fport 0 means MAC commands in FRMPayload.
        payload: Option<(u8, Vec<u8>)>,
    },
    JoinAccept {
        dl_settings: u8,
        rx_delay: u8,
        cf_list: Option<CfList>,
        /// Built with the wrong key, so the MIC does not check out.
        bad_mic: bool,
    },
    /// A frame built by the test; receives the network's FCntDown.
    Custom(FrameBuilder),
}

impl Pkt {
    fn data(payload: Option<(u8, Vec<u8>)>) -> Self {
        Self(PktKind::Data {
            frame_type: DataFrameType::UnconfirmedDown,
            ack: false,
            fopts: Vec::new(),
            payload,
        })
    }

    /// MAC commands in FRMPayload (FPort 0).
    pub fn mac(hex_payload: &str) -> Self {
        Self::data(Some((0, hex::decode(hex_payload).unwrap())))
    }

    /// Certification protocol command (FPort 224).
    pub fn cp_cmd(hex_payload: &str) -> Self {
        Self::data(Some((224, hex::decode(hex_payload).unwrap())))
    }

    /// Application data on `fport` (1..=223).
    pub fn app(fport: u8, payload: &[u8]) -> Self {
        assert!((1..=223).contains(&fport), "application FPort");
        Self::data(Some((fport, payload.to_vec())))
    }

    /// A frame without FPort and FRMPayload (eg a bare ACK).
    pub fn empty() -> Self {
        Self::data(None)
    }

    /// A JoinAccept for the test credentials, with default DLSettings and
    /// RxDelay and no CFList.
    pub fn join_accept() -> Self {
        Self(PktKind::JoinAccept { dl_settings: 0, rx_delay: 0, cf_list: None, bad_mic: false })
    }

    /// A frame the test builds itself from the FCntDown the network
    /// assigns (for frames the regular builders cannot express).
    pub fn custom(build: impl Fn(&mut [u8], u32) -> usize + Send + Sync + 'static) -> Self {
        Self(PktKind::Custom(Arc::new(build)))
    }

    /// Send a confirmed frame.
    pub fn confirmed(mut self) -> Self {
        let PktKind::Data { frame_type, .. } = &mut self.0 else {
            panic!("confirmed() applies to data frames")
        };
        *frame_type = DataFrameType::ConfirmedDown;
        self
    }

    /// Set the ACK bit (acknowledges the previous confirmed uplink).
    pub fn ack(mut self) -> Self {
        let PktKind::Data { ack, .. } = &mut self.0 else { panic!("ack() applies to data frames") };
        *ack = true;
        self
    }

    /// Piggyback MAC commands in FOpts.
    pub fn fopts(mut self, hex_cmds: &str) -> Self {
        let PktKind::Data { fopts, .. } = &mut self.0 else {
            panic!("fopts() applies to data frames")
        };
        *fopts = hex::decode(hex_cmds).unwrap();
        self
    }

    /// The JoinAccept's DLSettings byte (RX1DROffset, RX2DataRate).
    pub fn dl_settings(mut self, byte: u8) -> Self {
        let PktKind::JoinAccept { dl_settings, .. } = &mut self.0 else {
            panic!("dl_settings() applies to join accepts")
        };
        *dl_settings = byte;
        self
    }

    /// The JoinAccept's RxDelay, in seconds.
    pub fn rx_delay(mut self, seconds: u8) -> Self {
        let PktKind::JoinAccept { rx_delay, .. } = &mut self.0 else {
            panic!("rx_delay() applies to join accepts")
        };
        *rx_delay = seconds;
        self
    }

    /// The JoinAccept's CFList.
    pub fn cf_list(mut self, list: CfList) -> Self {
        let PktKind::JoinAccept { cf_list, .. } = &mut self.0 else {
            panic!("cf_list() applies to join accepts")
        };
        *cf_list = Some(list);
        self
    }

    /// Build the JoinAccept with the wrong key, so the device rejects it.
    pub fn bad_mic(mut self) -> Self {
        let PktKind::JoinAccept { bad_mic, .. } = &mut self.0 else {
            panic!("bad_mic() applies to join accepts")
        };
        *bad_mic = true;
        self
    }

    /// Whether the frame uses up a FCntDown of the network.
    fn consumes_fcnt_down(&self) -> bool {
        !matches!(self.0, PktKind::JoinAccept { .. })
    }

    /// Build the frame with the given FCntDown and session keys.
    fn build(&self, buf: &mut [u8], fcnt_down: u32, keys: &SessionKeys) -> usize {
        match &self.0 {
            PktKind::Data { frame_type, ack, fopts, payload } => {
                let frame = DataFrame {
                    frame_type: *frame_type,
                    dev_addr: get_dev_addr(),
                    ack: *ack,
                    fcnt: fcnt_down,
                    f_opts: fopts,
                    payload: match payload {
                        Some((0, data)) => Payload::MacCommands(data),
                        Some((fport, data)) => {
                            Payload::Data { f_port: NonZeroU8::new(*fport).unwrap(), data }
                        }
                        None => Payload::None,
                    },
                    ..Default::default()
                };
                let nwk = DefaultCrypto::new(keys.nwk.inner());
                let app = DefaultCrypto::new(keys.app.inner());
                frame.build_into(buf, &nwk, Some(&app)).unwrap().len()
            }
            PktKind::JoinAccept { dl_settings, rx_delay, cf_list, bad_mic } => {
                let accept = JoinAccept {
                    join_nonce: JoinNonce::from_wire_bytes([1; 3]),
                    net_id: NetId::from_wire_bytes([1; 3]),
                    dev_addr: get_dev_addr(),
                    dl_settings: DLSettings::new(*dl_settings),
                    rx_delay: *rx_delay,
                    c_f_list: cf_list.clone(),
                };
                let crypto = if *bad_mic {
                    DefaultNetworkCrypto::new(&[1; 16].into())
                } else {
                    get_network_crypto()
                };
                accept.build_into(buf, &crypto).unwrap().len()
            }
            PktKind::Custom(build) => build(buf, fcnt_down),
        }
    }
}

/// What the radio reports in an RX window.
enum Rx {
    Frame(Pkt),
    /// A receive error, the way lora-phy's adapter reports a frame whose
    /// header or payload CRC failed.
    PhyError(&'static str),
}

/// What the network sends during an exchange and in which window.
///
/// Windows are numbered across the whole exchange: 1 and 2 are the first
/// transmission's RX1 and RX2, 3 and 4 belong to the next transmission (an
/// NbTrans retransmission or a certification answer), and so on. Windows
/// without a scripted downlink time out. Every scripted downlink must get
/// delivered, or the exchange panics.
pub struct ExchangeAction {
    /// (window, event), one per window
    windows: Vec<(usize, Rx)>,
    /// (window, downlink) delivered into the RXC listen after that window
    /// closed (0 = right after the transmission), class C only.
    rxc: Vec<(usize, Pkt)>,
}

impl ExchangeAction {
    /// No downlink in any window: windows time out and NbTrans
    /// retransmissions run to completion.
    pub fn none() -> Self {
        Self { windows: Vec::new(), rxc: Vec::new() }
    }

    /// A downlink in the first transmission's RX1 window.
    pub fn rx1(pkt: Pkt) -> Self {
        Self::none().in_window(1, pkt)
    }

    /// A downlink in the first transmission's RX2 window.
    pub fn rx2(pkt: Pkt) -> Self {
        Self::none().in_window(2, pkt)
    }

    /// Also deliver `pkt` in `window` (numbered from 1, see above).
    pub fn in_window(self, window: usize, pkt: Pkt) -> Self {
        self.event(window, Rx::Frame(pkt))
    }

    /// The radio reports receive error `error` in `window` instead of a
    /// frame or a timeout.
    pub fn phy_error(self, window: usize, error: &'static str) -> Self {
        self.event(window, Rx::PhyError(error))
    }

    fn event(mut self, window: usize, rx: Rx) -> Self {
        assert!(window >= 1, "windows are numbered from 1");
        assert!(
            self.windows.iter().all(|(w, _)| *w != window),
            "window {window} already has an event"
        );
        self.windows.push((window, rx));
        self
    }

    /// Class C: also deliver `pkt` while the device listens in RXC after
    /// `window` closed (0 = between the transmission and RX1; after the
    /// last window the device listens during its RETRANSMIT_TIMEOUT wait).
    pub fn rxc_after(mut self, window: usize, pkt: Pkt) -> Self {
        assert!(
            self.rxc.iter().all(|(w, _)| *w != window),
            "the RXC listen after window {window} already has a downlink"
        );
        self.rxc.push((window, pkt));
        self
    }

    fn take(&mut self, window: usize) -> Option<Rx> {
        let i = self.windows.iter().position(|(w, _)| *w == window)?;
        Some(self.windows.swap_remove(i).1)
    }

    fn take_rxc(&mut self, window: usize) -> Option<Pkt> {
        let i = self.rxc.iter().position(|(w, _)| *w == window)?;
        Some(self.rxc.swap_remove(i).1)
    }
}

/// The outcome of a device operation driven by `Net`.
pub struct Outcome<R> {
    response: R,
    /// Every uplink transmitted during the exchange, in order (including
    /// retransmissions and certification answers).
    uplinks: Vec<Uplink>,
    /// The FCntDown the network used for its last downlink, if one was sent.
    fcnt_down: Option<u32>,
    /// The radio configuration of every RX window, in order.
    window_configs: Vec<RxConfig>,
}

pub type ExchangeResult = Outcome<Result<SendResponse, Error<&'static str>>>;
pub type JoinResult = Outcome<Result<JoinResponse, Error<&'static str>>>;
#[cfg(feature = "class-c")]
pub type ListenResult = Outcome<Result<ListenResponse, Error<&'static str>>>;

impl<R> Outcome<R> {
    /// The device's response, for checks the `expect_*` helpers do not cover.
    pub fn response(&self) -> &R {
        &self.response
    }

    /// The uplinks transmitted during the exchange, in order.
    pub fn uplinks(&self) -> &[Uplink] {
        &self.uplinks
    }

    pub fn uplink(&self, index: usize) -> &Uplink {
        &self.uplinks[index]
    }

    /// The FCntDown the network used for its last downlink, if one was sent.
    pub fn fcnt_down(&self) -> Option<u32> {
        self.fcnt_down
    }

    /// The number of RX windows the device opened.
    pub fn windows(&self) -> usize {
        self.window_configs.len()
    }

    /// The radio configuration the device used for `window` (numbered from
    /// 1, as in [`ExchangeAction`]).
    pub fn rx_config(&self, window: usize) -> RxConfig {
        assert!(window >= 1, "windows are numbered from 1");
        self.window_configs[window - 1]
    }
}

impl ExchangeResult {
    /// Assert the uplink was answered by a downlink.
    pub fn expect_downlink_received(&self) {
        match &self.response {
            Ok(SendResponse::DownlinkReceived(fcnt)) => {
                assert_eq!(*fcnt, self.fcnt_down.expect("a downlink was sent"), "FCntDown");
            }
            other => panic!("expected DownlinkReceived, got {other:?}"),
        }
    }

    /// Assert the uplink (and its retransmissions, if any) received no
    /// downlink and completed.
    pub fn expect_rx_complete(&self) {
        match &self.response {
            Ok(SendResponse::RxComplete) => {}
            other => panic!("expected RxComplete, got {other:?}"),
        }
    }

    /// Assert the confirmed uplink was never acknowledged.
    pub fn expect_no_ack(&self) {
        match &self.response {
            Ok(SendResponse::NoAck) => {}
            other => panic!("expected NoAck, got {other:?}"),
        }
    }
}

impl JoinResult {
    /// Assert the device joined.
    pub fn expect_joined(&self) {
        match &self.response {
            Ok(JoinResponse::JoinSuccess) => {}
            other => panic!("expected JoinSuccess, got {other:?}"),
        }
    }

    /// Assert the join timed out.
    pub fn expect_no_join_accept(&self) {
        match &self.response {
            Ok(JoinResponse::NoJoinAccept) => {}
            other => panic!("expected NoJoinAccept, got {other:?}"),
        }
    }
}

#[cfg(feature = "class-c")]
impl ListenResult {
    /// Assert the listen ended on a downlink for the application.
    pub fn expect_downlink_received(&self) {
        match &self.response {
            Ok(ListenResponse::DownlinkReceived(fcnt)) => {
                assert_eq!(*fcnt, self.fcnt_down.expect("a downlink was sent"), "FCntDown");
            }
            other => panic!("expected DownlinkReceived, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::util;
    use super::*;
    use crate::region;

    #[tokio::test]
    async fn exchange_delivers_downlink_in_rx1() {
        let (radio, timer, device) = util::session_with_region(region::EU868::new_eu868().into());
        let mut net = Net::new(radio, timer, device).await;
        // LinkADRReq, NbTrans = 2
        let r =
            net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0350070002"))).await;
        r.expect_downlink_received();
        assert_eq!(r.fcnt_down(), Some(0));
        assert_eq!(r.uplinks().len(), 1);
        assert!(!r.uplink(0).data().is_empty());
        assert_eq!(r.windows(), 1);
    }

    #[tokio::test]
    async fn exchange_no_downlink() {
        let (radio, timer, device) = util::session_with_region(region::EU868::new_eu868().into());
        let mut net = Net::new(radio, timer, device).await;
        let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
        r.expect_rx_complete();
        assert_eq!(r.fcnt_down(), None);
        assert_eq!(r.uplinks().len(), 1);
        assert_eq!(r.windows(), 2);
        // RX2 of EU868 is on 869.525 MHz
        assert_eq!(r.rx_config(2).rf.frequency, 869_525_000);
    }

    #[cfg(feature = "certification")]
    #[tokio::test]
    async fn exchange_downlink_in_rx2() {
        let (radio, timer, device) = util::session_with_region(region::EU868::new_eu868().into());
        let mut net = Net::new(radio, timer, device).await;
        let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx2(Pkt::cp_cmd("09"))).await;
        // The RxAppCntAns is a regular uplink with its own (silent) RX
        // windows, then RxComplete.
        r.expect_rx_complete();
        assert_eq!(r.fcnt_down(), Some(0));
        // the send uplink plus the certification answer
        assert_eq!(r.uplinks().len(), 2);
    }

    #[tokio::test]
    async fn exchange_downlink_in_a_retransmission_window_ends_the_cycle() {
        let (radio, timer, mut device) =
            util::session_with_region(region::EU868::new_eu868().into());
        device.mac.configuration.nb_trans = 3;
        let mut net = Net::new(radio, timer, device).await;
        // First copy: RX1 and RX2 silent. Second copy: LinkADRReq in RX1.
        let action = ExchangeAction::none().in_window(3, Pkt::mac("0350070001"));
        let r = net.exchange(&[1, 2, 3], 3, false, action).await;
        r.expect_downlink_received();
        // two copies, the third is not sent
        assert_eq!(r.uplinks().len(), 2);
        assert_eq!(r.uplink(0).data(), r.uplink(1).data());
    }

    #[tokio::test]
    async fn exchange_ack_in_a_repetition_window_ends_the_confirmed_cycle() {
        let (radio, timer, mut device) =
            util::session_with_region(region::EU868::new_eu868().into());
        device.mac.configuration.nb_trans = 3;
        let mut net = Net::new(radio, timer, device).await;
        // First copy unacknowledged, the second one acknowledged in RX2.
        let action = ExchangeAction::none().in_window(4, Pkt::mac("0350070001").ack());
        let r = net.exchange(&[1, 2, 3], 3, true, action).await;
        r.expect_downlink_received();
        assert_eq!(r.uplinks().len(), 2);
    }

    #[tokio::test]
    #[should_panic(expected = "scripted downlink for window 3 was never delivered")]
    async fn exchange_panics_on_an_undelivered_downlink() {
        let (radio, timer, device) = util::session_with_region(region::EU868::new_eu868().into());
        let mut net = Net::new(radio, timer, device).await;
        // NbTrans = 1: there is no window 3
        let action = ExchangeAction::none().in_window(3, Pkt::mac("0350070001"));
        net.exchange(&[1, 2, 3], 3, false, action).await;
    }

    #[tokio::test]
    async fn join_resets_the_downlink_counter() {
        let (radio, timer, device) = util::setup();
        let mut net = Net::new(radio, timer, device).await;
        let r = net.join(ExchangeAction::rx1(Pkt::join_accept())).await;
        r.expect_joined();
        assert_eq!(r.fcnt_down(), None, "a join accept carries no FCntDown");
        let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("06"))).await;
        r.expect_downlink_received();
        assert_eq!(r.fcnt_down(), Some(0));
    }

    #[tokio::test]
    async fn exchange_ignores_uplinks_sent_before_the_network_took_over() {
        let (radio, timer, mut device) =
            util::session_with_region(region::EU868::new_eu868().into());
        // An uplink driven by hand, before the network takes over.
        let task = tokio::spawn(async move {
            let response = device.send(&[3, 2, 1], 3, false).await;
            (device, response)
        });
        timer.fire_most_recent().await; // RX1 start
        radio.handle_timeout().await; // RX1 end
        timer.fire_most_recent().await; // RX2 start
        radio.handle_timeout().await; // RX2 end
        let (device, _) = task.await.unwrap();

        let mut net = Net::new(radio, timer, device).await;
        let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
        r.expect_rx_complete();
        assert_eq!(r.uplinks().len(), 1);
        assert_eq!(r.windows(), 2);
    }

    #[cfg(feature = "class-c")]
    #[tokio::test]
    async fn exchange_class_c_confirmed_no_ack() {
        let mut net = util::setup_with_session_class_c().await;
        net.device().mac.configuration.nb_trans = 3;
        // Each RETRANSMIT_TIMEOUT wait is spent listening in RXC.
        let r = net.exchange(&[1, 2, 3], 3, true, ExchangeAction::none()).await;
        r.expect_no_ack();
        assert_eq!(r.uplinks().len(), 3);
        assert_eq!(r.uplink(0).data(), r.uplink(1).data());
        assert_eq!(r.uplink(1).data(), r.uplink(2).data());
    }

    #[cfg(feature = "class-c")]
    #[tokio::test]
    async fn exchange_continues_the_session_fcnt_down() {
        // The class C helper already delivered FCntDown 0.
        let mut net = util::setup_with_session_class_c().await;
        assert_eq!(net.device().mac.get_session().unwrap().fcnt_down(), Some(0));
        // LinkADRReq, NbTrans = 1
        let r =
            net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0350070001"))).await;
        r.expect_downlink_received();
        assert_eq!(r.fcnt_down(), Some(1));
    }

    #[tokio::test]
    #[should_panic(expected = "rxc_after needs a class C device")]
    async fn exchange_rejects_an_rxc_downlink_for_a_class_a_device() {
        let (radio, timer, device) = util::session_with_region(region::EU868::new_eu868().into());
        let mut net = Net::new(radio, timer, device).await;
        let action = ExchangeAction::none().rxc_after(0, Pkt::app(3, &[1]));
        net.exchange(&[1, 2, 3], 3, false, action).await;
    }

    #[cfg(feature = "class-c")]
    #[tokio::test]
    #[should_panic(expected = "scripted RXC downlink after window 2 was never delivered")]
    async fn exchange_panics_on_an_undelivered_rxc_downlink() {
        let mut net = util::setup_with_session_class_c().await;
        // An unconfirmed uplink has no RETRANSMIT_TIMEOUT wait after RX2.
        let action = ExchangeAction::none().rxc_after(2, Pkt::app(3, &[1]));
        net.exchange(&[1, 2, 3], 3, false, action).await;
    }
}
