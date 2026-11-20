//! Shared machinery for the paired-scenario suites: the network-side fixture
//! wrapper (`Net`) and the scenario drivers. Each bench DUT gets its own thin
//! suite binary (scenarios.rs = NUCLEO-WL55JC, scenarios_l072.rs =
//! B-L072Z-LRWAN1) registering the same drivers against its firmware crate.

use banc_host::expect::expect_matching;
use banc_host::{DeviceSuite, DeviceTest, Failed, TestCx};
use lora_hil_harness::{
    Harness, HarnessEvent, JoinAcceptParams, JoinPolicy, RxWindow, UplinkResponse,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

/// Firmware crate to build/flash; the env var overrides for CI, where the
/// device tests come from the workflow's checkout rather than the bench one.
/// --offline is the bench default (the lounas sandbox hangs cargo's network
/// probe); HIL_FW_OFFLINE=0 drops it for the CI container, which has no
/// registry cache on purpose and fetches from crates.io.
pub fn device_suite(env_var: &str, default_crate_dir: String) -> DeviceSuite {
    let fw_crate = std::env::var(env_var).unwrap_or(default_crate_dir);
    let mut suite = DeviceSuite::new(fw_crate, "join");
    suite.cargo_args.push("--release".into());
    if std::env::var("HIL_FW_OFFLINE").map(|v| v != "0").unwrap_or(true) {
        suite.cargo_args.push("--offline".into());
    }
    suite
}

/// The network-side fixture for one scenario: the Semtech-UDP harness running
/// as a task, its events teed into banc evidence on the way to the test.
/// Dropping it aborts the harness task, releasing port 1730 for the next
/// scenario (they share one process under the banc runner).
pub struct Net {
    pub events: UnboundedReceiver<HarnessEvent>,
    harness_task: tokio::task::JoinHandle<()>,
    /// On remote rigs (a "relay" assistant in the rig config), the pump
    /// carrying gateway datagrams between the rig daemon and our local
    /// harness socket. Dropped with the scenario, like the harness.
    _relay: Option<lora_hil_harness::relay::RelayPump>,
}

impl Drop for Net {
    fn drop(&mut self) {
        self.harness_task.abort();
    }
}

impl Net {
    pub async fn bind(
        cx: &TestCx,
        policy: JoinPolicy,
        join_params: Option<JoinAcceptParams>,
        uplink_response: Option<UplinkResponse>,
    ) -> Result<Net, Failed> {
        let addr = SocketAddr::from(([0, 0, 0, 0], 1730));
        // The previous scenario's harness task releases the socket at its
        // first poll after abort; absorb that race with a short retry.
        let mut attempt = 0;
        let (mut harness, raw_events) = loop {
            // The bind error is not Send; stringify it before awaiting.
            let err = match Harness::bind(addr).await {
                Ok(pair) => break pair,
                Err(e) => e.to_string(),
            };
            if attempt >= 10 {
                return Err(Failed::from(format!(
                    "cannot bind :1730 (standalone harness still running?): {err}"
                )));
            }
            attempt += 1;
            tokio::time::sleep(Duration::from_millis(200)).await;
        };
        harness.policy = policy;
        if let Some(params) = join_params {
            harness.join_params = params;
        }
        harness.uplink_response = uplink_response;
        let harness_task = tokio::spawn(async move { harness.run().await });

        // Remote rig: gateway datagrams arrive via the rig daemon rather
        // than directly on :1730. The pump is per-scenario like the harness;
        // the daemon replays its peer table to each fresh connection.
        let relay = match cx.rig.config.assistant("relay") {
            Some(_) => {
                let node = cx
                    .rig
                    .assistant("relay")
                    .await
                    .map_err(|e| Failed::from(format!("connecting to rig daemon: {e}")))?;
                let harness_addr = SocketAddr::from(([127, 0, 0, 1], 1730));
                Some(
                    lora_hil_harness::relay::RelayPump::start(node, harness_addr)
                        .await
                        .map_err(|e| Failed::from(format!("starting relay pump: {e}")))?,
                )
            }
            None => None,
        };

        // Tee every fixture event into evidence so a failing scenario dumps
        // the full network-side narrative.
        let (tx, events) = tokio::sync::mpsc::unbounded_channel();
        let evidence = cx.evidence.clone();
        let mut raw_events = raw_events;
        tokio::spawn(async move {
            while let Some(ev) = raw_events.recv().await {
                evidence.record("net", format!("{ev:?}"));
                let _ = tx.send(ev);
            }
        });

        let mut net = Net { events, harness_task, _relay: relay };

        // Downlink-readiness gate: after a rebind the gateway can deliver
        // uplinks (PUSH_DATA) before it has registered its downlink socket
        // (PULL_DATA keepalive, ~10 s period). A JoinAccept dispatched in
        // that window dies with AckRecv and burns a device join attempt —
        // seen live 2026-08-03 (paired_join_dlsettings_rx2: JoinAcceptSent
        // at 6.8 s failed, GatewayConnected only at 7.5 s). Wait for the
        // registration before letting the scenario proceed.
        net.expect("gateway PULL_DATA registration", Duration::from_secs(30), |e| {
            matches!(e, HarnessEvent::GatewayConnected { .. })
        })
        .await?;

        Ok(net)
    }

    /// Wait for an event matching `pred`, failing the scenario on deadline.
    /// Non-matching events are skipped (busy RF neighborhood); all of them
    /// are already in evidence via the tee.
    pub async fn expect(
        &mut self,
        what: &str,
        deadline: Duration,
        pred: impl FnMut(&HarnessEvent) -> bool,
    ) -> Result<HarnessEvent, Failed> {
        expect_matching(&mut self.events, deadline, pred)
            .await
            .map_err(|e| Failed::from(format!("waiting for {what}: {e}")))
    }
}

pub async fn join_ok(cx: &TestCx, device: &mut DeviceTest) -> Result<(), Failed> {
    let mut net = Net::bind(cx, JoinPolicy::Accept, None, None).await?;

    // Flash + boot + first join attempt: generous deadline, the ELF transfer
    // and flash alone take ~20 s.
    device.start();

    net.expect("JoinRequest", Duration::from_secs(120), |e| {
        matches!(e, HarnessEvent::JoinRequestReceived { .. })
    })
    .await?;
    net.expect("JoinAccept dispatch", Duration::from_secs(10), |e| {
        matches!(e, HarnessEvent::JoinAcceptSent { .. })
    })
    .await?;
    net.expect("gateway TX_ACK", Duration::from_secs(10), |e| {
        matches!(e, HarnessEvent::DownlinkAcked | HarnessEvent::DownlinkFailed { .. })
    })
    .await?;

    // The device test sends one uplink right after joining; it may need a
    // couple of join attempts on a bad RF day, so allow for retries.
    let hb = net
        .expect("decrypted heartbeat", Duration::from_secs(180), |e| {
            matches!(e, HarnessEvent::HeartbeatDecrypted { .. })
        })
        .await?;
    let HarnessEvent::HeartbeatDecrypted { fcnt, fport, payload } = hb else { unreachable!() };
    if fcnt != 0 {
        return Err(format!("expected first frame of a fresh session, got FCnt {fcnt}").into());
    }
    if fport != 1 {
        return Err(format!("expected heartbeat on fport 1, got fport {fport}").into());
    }
    if payload != [0xC0, 0xFF, 0xEE, 0x01] {
        return Err(format!("unexpected heartbeat payload: {payload:02X?}").into());
    }
    // Device-side verdict (semihosting exit via probe-rs) is awaited by the
    // suite after this body returns.
    Ok(())
}

/// Negative-scenario driver: the device must never join and, critically, no
/// heartbeat may decrypt. With TamperMic the fixture still stores the honest
/// session keys, so a wrongly-accepted join betrays itself by producing
/// decryptable heartbeats; their absence is the network-side verdict.
pub async fn join_never(
    cx: &TestCx,
    device: &mut DeviceTest,
    policy: JoinPolicy,
) -> Result<(), Failed> {
    let expect_accepts = matches!(policy, JoinPolicy::TamperMic);
    let mut net = Net::bind(cx, policy, None, None).await?;

    device.start();

    net.expect("JoinRequest", Duration::from_secs(120), |e| {
        matches!(e, HarnessEvent::JoinRequestReceived { .. })
    })
    .await?;
    if expect_accepts {
        // The tampered accept must actually go out over the air for the
        // device-side rejection to mean anything.
        net.expect("tampered JoinAccept dispatch", Duration::from_secs(10), |e| {
            matches!(e, HarnessEvent::JoinAcceptSent { .. })
        })
        .await?;
        net.expect("gateway TX_ACK", Duration::from_secs(10), |e| {
            matches!(e, HarnessEvent::DownlinkAcked | HarnessEvent::DownlinkFailed { .. })
        })
        .await?;
    }

    // The device exhausts its attempts and reports its own verdict; wait for
    // it here so everything the fixture will ever see is queued before the
    // drain below.
    device.verdict().await?;

    // A decrypted heartbeat anywhere means the DUT accepted a join it must
    // not have.
    while let Ok(ev) = net.events.try_recv() {
        if matches!(ev, HarnessEvent::HeartbeatDecrypted { .. }) {
            return Err(
                format!("heartbeat decrypted during a negative join scenario: {ev:?}").into()
            );
        }
        if !expect_accepts && matches!(ev, HarnessEvent::JoinAcceptSent { .. }) {
            return Err("harness sent a JoinAccept under JoinPolicy::Ignore".into());
        }
    }
    Ok(())
}

/// Downlink used by the DLSettings scenarios (the device tests mirror these
/// in tests-hil-fw*/tests/join.rs; change them together).
const MAGIC_DOWNLINK: [u8; 3] = [0xBE, 0xEF, 0x42];
const MAGIC_FPORT: u8 = 42;

/// DLSettings variant driver (hardware validation for lora-rs PR #461): the
/// JoinAccept carries nonstandard RX parameters and the harness answers
/// heartbeats with a downlink timed/datarated per those parameters. The
/// device-side verdict is receiving that downlink; a device that did not
/// adopt the accept's DLSettings/RxDelay listens in the wrong window.
pub async fn join_dlsettings(
    cx: &TestCx,
    device: &mut DeviceTest,
    params: JoinAcceptParams,
    window: RxWindow,
) -> Result<(), Failed> {
    let uplink = UplinkResponse { payload: MAGIC_DOWNLINK.to_vec(), fport: MAGIC_FPORT, window };
    let mut net = Net::bind(cx, JoinPolicy::Accept, Some(params), Some(uplink)).await?;

    device.start();

    net.expect("JoinRequest", Duration::from_secs(120), |e| {
        matches!(e, HarnessEvent::JoinRequestReceived { .. })
    })
    .await?;
    net.expect("JoinAccept dispatch", Duration::from_secs(10), |e| {
        matches!(e, HarnessEvent::JoinAcceptSent { .. })
    })
    .await?;
    net.expect("decrypted heartbeat", Duration::from_secs(180), |e| {
        matches!(e, HarnessEvent::HeartbeatDecrypted { .. })
    })
    .await?;
    net.expect("data downlink scheduled", Duration::from_secs(10), |e| {
        matches!(e, HarnessEvent::DataDownlinkScheduled { .. })
    })
    .await?;
    net.expect("gateway TX_ACK for data downlink", Duration::from_secs(10), |e| {
        matches!(e, HarnessEvent::DownlinkAcked | HarnessEvent::DownlinkFailed { .. })
    })
    .await?;

    // Device-side verdict: it received and verified the magic downlink;
    // awaited by the suite after this body returns.
    Ok(())
}
