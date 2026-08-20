//! Two-sided HIL scenarios for the B-L072Z-LRWAN1 DUT (SX1276 via the Murata
//! CMWX1ZZABZ module): the same five scenarios as scenarios.rs, driving the
//! on-target embedded-tests in ../tests-hil-fw-l072. See scenarios.rs for the
//! suite mechanics; the two differ only in which firmware crate they flash.
//!
//! This DUT joins as DevEUI 00..02 (same AppKey), so bench captures can tell
//! the boards apart. Scenarios from both suites serialize on the banc rig
//! lock; only one DUT is ever active, the other sits halted on its probe.

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
        "HIL_FW_CRATE_L072",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../tests-hil-fw-l072").into(),
    )
}
