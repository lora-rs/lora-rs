//! A network-side test harness ("TCL") for driving `Device::send`.
//!
//! `Net` wraps the radio channel, the timer channel and the device under
//! test, like a network and its radio would.

use super::{
    Device,
    radio::RadioChannel,
    timer::{TimerChannel, TimerKind},
};
use crate::async_device::{Error, SendResponse};
use crate::test_util::{Uplink, get_crypto, get_dev_addr};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, Payload};
use lorawan::parser::DataFrameType;

pub struct Net {
    radio: RadioChannel,
    timer: TimerChannel,
    device: Option<Device>,
    /// FCntDown of the network's next downlink, assigned by network.
    fcnt_down: u32,
}

impl Net {
    pub async fn new(radio: RadioChannel, timer: TimerChannel, device: Device) -> Self {
        // Uplinks the device transmitted before the network took over (eg
        // the one that enabled class C) belong to no exchange.
        radio.take_uplinks().await;
        // Continue the session's downlink counter: a repeated FCntDown is a
        // replay and the device drops the frame.
        let fcnt_down = device.mac.get_session().and_then(|s| s.fcnt_down()).map_or(0, |f| f + 1);
        Self { radio, timer, device: Some(device), fcnt_down }
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
        mut action: ExchangeAction,
    ) -> ExchangeResult {
        let data = data.to_vec(); // the spawned task owns the payload
        let baseline = self.timer.get_armed_count().await;
        let device = self.device.take().expect("device not in use");
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_flag = done.clone();
        let task = tokio::spawn(async move {
            let mut device = device;
            let response = device.send(&data, fport, confirmed).await;
            done_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            (device, response)
        });
        let is_done = || done.load(std::sync::atomic::Ordering::SeqCst);

        // Drive the radio:
        // a) wait for the device to arm a timer (a rx window or a retransmission wait)
        // b) fire timer
        // c) once window is about to open:
        // - deliver the scripted downlink or...
        // - timeout
        // A retransmission wait is not followed by a radio read, so no
        // radio event is sent for it.
        // Repeat until `send` returns.
        let mut last = baseline;
        let mut window = 0;
        let mut fcnt_down = None;
        loop {
            let mut armed = self.timer.get_armed_count().await;
            let wait_start = std::time::Instant::now();
            while armed == last && !is_done() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                if wait_start.elapsed() > std::time::Duration::from_secs(5) {
                    panic!(
                        "exchange stuck: waiting for a timer arm or send to finish; \
window={window} last={last} armed={armed}"
                    );
                }
                if is_done() {
                    break;
                }
                armed = self.timer.get_armed_count().await;
            }
            if is_done() {
                break;
            }
            last = armed;
            // Read the kind before firing:
            // firing wakes the device, this can arm the next timer and
            // overwrite the kind.
            let kind = self.timer.last_timer_kind();
            self.timer.fire_most_recent().await;
            if kind == TimerKind::Delay {
                continue;
            }
            window += 1;

            if let Some(pkt) = action.take(window) {
                let fcnt = self.fcnt_down;
                let mut buf = [0u8; 256];
                let len = pkt.build(&mut buf, fcnt);
                self.radio.handle_frame(buf[..len].to_vec()).await;
                self.fcnt_down += 1;
                fcnt_down = Some(fcnt);
            } else {
                self.radio.handle_timeout().await;
            }
        }

        let (device, response) = task.await.expect("device task panicked");
        if let Some(w) = action.first_pending() {
            panic!(
                "scripted downlink for window {w} was never delivered: the exchange ended after {window} windows"
            );
        }
        let uplinks = self.radio.take_uplinks().await;
        self.device = Some(device);
        ExchangeResult { response, uplinks, fcnt_down }
    }
}

/// A downlink to be delivered by the network.
/// It carries no FCntDown: the network assigns it from its own counter
/// when the frame is built.
#[derive(Clone)]
pub struct Pkt {
    frame_type: DataFrameType,
    ack: bool,
    f_opts: Option<Vec<u8>>,
    /// (fport, payload); fport 0 means MAC commands in FRMPayload.
    payload: Option<(u8, Vec<u8>)>,
}

impl Pkt {
    fn new(payload: Option<(u8, Vec<u8>)>) -> Self {
        Self { frame_type: DataFrameType::UnconfirmedDown, ack: false, f_opts: None, payload }
    }

    /// MAC commands in FRMPayload (FPort 0).
    pub fn mac(hex_payload: &str) -> Self {
        Self::new(Some((0, hex::decode(hex_payload).unwrap())))
    }

    /// Certification protocol command (FPort 224).
    pub fn cp_cmd(hex_payload: &str) -> Self {
        Self::new(Some((224, hex::decode(hex_payload).unwrap())))
    }

    /// Send a confirmed frame.
    pub fn confirmed(mut self) -> Self {
        self.frame_type = DataFrameType::ConfirmedDown;
        self
    }

    /// Set the ACK bit (acknowledges the previous confirmed uplink).
    pub fn ack(mut self) -> Self {
        self.ack = true;
        self
    }

    /// Build the frame with the given FCntDown.
    pub fn build(&self, buf: &mut [u8], fcnt_down: u32) -> usize {
        let frame = DataFrame {
            frame_type: self.frame_type,
            dev_addr: get_dev_addr(),
            ack: self.ack,
            fcnt: fcnt_down,
            f_opts: self.f_opts.as_deref().unwrap_or_default(),
            payload: match &self.payload {
                Some((0, data)) => Payload::MacCommands(data),
                Some((fport, data)) => {
                    Payload::Data { f_port: NonZeroU8::new(*fport).unwrap(), data }
                }
                None => Payload::None,
            },
            ..Default::default()
        };
        frame.build_into(buf, &get_crypto(), Some(&get_crypto())).unwrap().len()
    }
}

/// What the network sends during an exchange and in which window.
///
/// Windows are numbered across the whole exchange: 1 and 2 are the first
/// transmission's RX1 and RX2, 3 and 4 belong to the next transmission (an
/// NbTrans retransmission or a certification answer), and so on. Windows
/// without a scripted downlink time out. Every scripted downlink must get
/// delivered, or the exchange panics.
pub struct ExchangeAction {
    /// (window, downlink), one per window
    downlinks: Vec<(usize, Pkt)>,
}

impl ExchangeAction {
    /// No downlink in any window: windows time out and NbTrans
    /// retransmissions run to completion.
    pub fn none() -> Self {
        Self { downlinks: Vec::new() }
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
    pub fn in_window(mut self, window: usize, pkt: Pkt) -> Self {
        assert!(window >= 1, "windows are numbered from 1");
        assert!(
            self.downlinks.iter().all(|(w, _)| *w != window),
            "window {window} already has a downlink"
        );
        self.downlinks.push((window, pkt));
        self
    }

    fn take(&mut self, window: usize) -> Option<Pkt> {
        let i = self.downlinks.iter().position(|(w, _)| *w == window)?;
        Some(self.downlinks.swap_remove(i).1)
    }

    /// The lowest window still waiting for its downlink.
    fn first_pending(&self) -> Option<usize> {
        self.downlinks.iter().map(|(w, _)| *w).min()
    }
}

/// The outcome of `Net::exchange`.
pub struct ExchangeResult {
    response: Result<SendResponse, Error<&'static str>>,
    /// Every uplink transmitted during the exchange, in order (including
    /// retransmissions and certification answers).
    uplinks: Vec<Uplink>,
    /// The FCntDown the network used for its last downlink, if one was sent.
    fcnt_down: Option<u32>,
}

impl ExchangeResult {
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
    }

    #[tokio::test]
    async fn exchange_no_downlink() {
        let (radio, timer, device) = util::session_with_region(region::EU868::new_eu868().into());
        let mut net = Net::new(radio, timer, device).await;
        let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
        r.expect_rx_complete();
        assert_eq!(r.fcnt_down(), None);
        assert_eq!(r.uplinks().len(), 1);
    }

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

    #[cfg(feature = "class-c")]
    #[tokio::test]
    async fn exchange_ignores_uplinks_sent_before_the_network_took_over() {
        // The class C helper sends one uplink of its own to enable class C.
        let (radio, timer, device) = util::setup_with_session_class_c().await;
        let mut net = Net::new(radio, timer, device).await;
        let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
        r.expect_rx_complete();
        assert_eq!(r.uplinks().len(), 1);
    }

    #[cfg(feature = "class-c")]
    #[tokio::test]
    async fn exchange_continues_the_session_fcnt_down() {
        // The class C helper already delivered FCntDown 0.
        let (radio, timer, device) = util::setup_with_session_class_c().await;
        assert_eq!(device.mac.get_session().unwrap().fcnt_down(), Some(0));
        let mut net = Net::new(radio, timer, device).await;
        // LinkADRReq, NbTrans = 1
        let r =
            net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0350070001"))).await;
        r.expect_downlink_received();
        assert_eq!(r.fcnt_down(), Some(1));
    }
}
