//! Two-sided HIL scenarios for the NUCLEO-WL55JC DUT: each test pairs the
//! network-side fixture with an on-target embedded-test in ../tests-hil-fw,
//! invoked over the cargo runner (probe-rs on the lora-hil Pi).
//!
//! Runs on the banc harness (github.com/lthiery/banc) via `paired_suite!`:
//! each scenario is declared next to the name of its device-side twin, gets a
//! `DeviceTest` handle it starts once its fixture is up, and cannot pass
//! without the device-side verdict (the suite awaits it after every body).
//! On a machine without the rig (no banc-rig.toml/BANC_RIG) every scenario reports
//! ignored instead of failing; on the rig, access is serialized by a
//! cross-process file lock and all fixture events are attached as evidence
//! when a scenario fails.
//!
//! Requirements on the rig: gateway pkt_fwd up, DUT connected, port 1730
//! free (no standalone harness bound to it).

mod common;

use common::{join_dlsettings, join_never, join_ok};
use banc_host::DeviceSuite;
use lora_hil_harness::{JoinAcceptParams, JoinPolicy, RxWindow};

banc_host::paired_suite! {
    device_suite: device_suite(),
    scenario paired_join_ok, device_test: "tests::join_ok", |cx, device| {
        join_ok(&cx, &mut device).await?;
    }
    scenario paired_join_tamper_mic, device_test: "tests::join_never_accepted", |cx, device| {
        join_never(&cx, &mut device, JoinPolicy::TamperMic).await?;
    }
    scenario paired_join_ignored, device_test: "tests::join_never_accepted", |cx, device| {
        join_never(&cx, &mut device, JoinPolicy::Ignore).await?;
    }
    scenario paired_join_dlsettings_rx1, device_test: "tests::join_dlsettings_rx1", |cx, device| {
        // RX1DROffset 2 (DR0 uplink -> DR8 downlink instead of DR10),
        // RxDelay 3.
        join_dlsettings(
            &cx,
            &mut device,
            JoinAcceptParams { dl_settings: 0x28, rx_delay: 3 },
            RxWindow::Rx1,
        )
        .await?;
    }
    scenario paired_join_dlsettings_rx2, device_test: "tests::join_dlsettings_rx2", |cx, device| {
        // RX2 DR10 instead of the US915 default DR8.
        join_dlsettings(
            &cx,
            &mut device,
            JoinAcceptParams { dl_settings: 0x0A, rx_delay: 1 },
            RxWindow::Rx2,
        )
        .await?;
    }
}

fn device_suite() -> DeviceSuite {
    common::device_suite(
        "HIL_FW_CRATE",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../tests-hil-fw").into(),
    )
}
