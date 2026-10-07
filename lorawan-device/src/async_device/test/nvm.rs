//! Tests for LoRaWAN 1.0.4 persistence: monotonic DevNonce with commit-before-transmit,
//! JoinNonce replay rejection, session resume across power loss, checkpoint wear-leveling
//! and write dedup. "Power loss" is simulated by dropping the device and calling
//! [`Device::restore`] against the same mock store.

use super::radio::TestRadio;
use super::timer::TestTimer;
use super::{JoinResponse, SendResponse, region};
use crate::mac::{NetworkCredentials, Session};
use crate::nvm::{
    MAX_BLOB_LEN, NonVolatileStoreSync, NvmRegion, PersistentIdentity, PersistentSession,
};
#[cfg(feature = "certification")]
use crate::radio::RfConfig;
use crate::test_util::{
    Uplink, get_abp_credentials, get_crypto, get_key, get_network_crypto, get_otaa_credentials,
};
use crate::{AppEui, AppKey, DevEui};
use core::num::NonZeroU8;
use lorawan::creator::{DataFrame, JoinAccept, Payload};
use lorawan::default_crypto::DefaultCrypto;
use lorawan::parser::{
    self, DataFrameType, DecryptedJoinAcceptPayload, JoinNonce, NetId, PhyPayload,
};
use lorawan::types::DLSettings;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, LazyLock, Mutex};

type Device =
    crate::async_device::Device<TestRadio, TestTimer, rand_core::OsRng, 512, 4, MockStore>;

#[derive(Default)]
struct Inner {
    slots: [Option<Vec<u8>>; 2],
    saves: [usize; 2],
}

/// In-memory store implementing the *sync* trait; the device consumes it through the
/// blanket async impl, so these tests cover that path too.
#[derive(Clone, Default)]
struct MockStore {
    inner: Arc<Mutex<Inner>>,
}

fn slot(region: NvmRegion) -> usize {
    match region {
        NvmRegion::Identity => 0,
        NvmRegion::Session => 1,
    }
}

impl MockStore {
    fn blob(&self, region: NvmRegion) -> Option<Vec<u8>> {
        self.inner.lock().unwrap().slots[slot(region)].clone()
    }

    fn saves(&self, region: NvmRegion) -> usize {
        self.inner.lock().unwrap().saves[slot(region)]
    }

    fn identity(&self) -> PersistentIdentity {
        PersistentIdentity::decode(&self.blob(NvmRegion::Identity).unwrap()).unwrap()
    }

    fn session(&self) -> PersistentSession {
        PersistentSession::decode(&self.blob(NvmRegion::Session).unwrap()).unwrap()
    }

    fn put(&self, region: NvmRegion, bytes: &[u8]) {
        self.inner.lock().unwrap().slots[slot(region)] = Some(bytes.to_vec());
    }

    fn corrupt(&self, region: NvmRegion) {
        let mut inner = self.inner.lock().unwrap();
        let blob = inner.slots[slot(region)].as_mut().unwrap();
        *blob.last_mut().unwrap() ^= 0x01;
    }
}

impl NonVolatileStoreSync for MockStore {
    type Error = Infallible;

    fn save(&mut self, region: NvmRegion, bytes: &[u8]) -> Result<(), Infallible> {
        let mut inner = self.inner.lock().unwrap();
        inner.slots[slot(region)] = Some(bytes.to_vec());
        inner.saves[slot(region)] += 1;
        Ok(())
    }

    fn load(&mut self, region: NvmRegion, buf: &mut [u8]) -> Result<Option<usize>, Infallible> {
        Ok(self.inner.lock().unwrap().slots[slot(region)].as_ref().map(|blob| {
            buf[..blob.len()].copy_from_slice(blob);
            blob.len()
        }))
    }
}

async fn restore(
    store: &MockStore,
) -> (super::radio::RadioChannel, super::timer::TimerChannel, Device) {
    let (radio_channel, mock_radio) = TestRadio::new();
    let (timer_channel, mock_timer) = TestTimer::new();
    let region = region::US915::default();
    let device =
        Device::restore(region.into(), mock_radio, mock_timer, rand_core::OsRng, store.clone())
            .await
            .unwrap();
    (radio_channel, timer_channel, device)
}

/// Sessions derived by the "network side" during joins, keyed by test id, so data
/// handlers can validate and encrypt against the real derived keys.
static NETWORK_SESSIONS: LazyLock<Mutex<HashMap<usize, Session>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Network-side join handler: asserts the transmitted DevNonce equals `EXPECT_DEV_NONCE`
/// (monotonic counter), answers with JoinNonce `NONCE` and stores the derived session
/// under test id `T`.
fn join_accept<const T: usize, const NONCE: u8, const EXPECT_DEV_NONCE: u16>(
    uplink: Option<Uplink>,
    _config: crate::radio::RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let mut uplink = uplink.expect("no uplink");
    let Ok(PhyPayload::JoinRequest(join_request)) = parser::parse(uplink.data_mut()) else {
        panic!("not a join request");
    };
    assert!(join_request.validate_mic(&get_crypto()));
    let dev_nonce = join_request.dev_nonce();
    assert_eq!(dev_nonce.value(), EXPECT_DEV_NONCE, "DevNonce not the expected counter value");

    let accept = JoinAccept {
        join_nonce: JoinNonce::from_wire_bytes([NONCE, 0, 0]),
        net_id: NetId::from_wire_bytes([1; 3]),
        dev_addr: crate::test_util::get_dev_addr(),
        dl_settings: DLSettings::new(0),
        rx_delay: 0,
        c_f_list: None,
    };
    let finished = accept.build_into(rx_buffer, &get_network_crypto()).unwrap();
    let len = finished.len();

    let mut copy = finished.to_vec();
    let decrypt = DecryptedJoinAcceptPayload::check_mic_and_decrypt_in_place(
        copy.as_mut_slice(),
        &get_crypto(),
    )
    .expect("could not parse own join accept");
    let session = Session::derive_new(
        &decrypt,
        dev_nonce,
        &NetworkCredentials::new(
            AppEui::from([0; 8]),
            DevEui::from([0; 8]),
            AppKey::from(get_key()),
        ),
    );
    NETWORK_SESSIONS.lock().unwrap().insert(T, session);
    len
}

/// Network-side data handler: validates the uplink against the session derived at join
/// time for test id `T`, asserts its FCnt, and answers with a downlink at `FCNT_DOWN`.
fn data_downlink<const T: usize, const FCNT_UP: u16, const FCNT_DOWN: u32>(
    uplink: Option<Uplink>,
    _config: crate::radio::RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let session = NETWORK_SESSIONS.lock().unwrap().get(&T).cloned().unwrap();
    let nwk = DefaultCrypto::new(session.nwkskey().inner());
    let app = DefaultCrypto::new(session.appskey().inner());
    let mut uplink = uplink.expect("no uplink");
    let Ok(PhyPayload::Data(data)) = parser::parse(uplink.data_mut()) else {
        panic!("not a data payload");
    };
    let fcnt = data.fhdr().fcnt() as u32;
    assert!(data.validate_mic(&nwk, fcnt));
    assert_eq!(fcnt as u16, FCNT_UP, "uplink FCnt mismatch");

    let frame = DataFrame {
        frame_type: DataFrameType::UnconfirmedDown,
        dev_addr: *session.devaddr(),
        fcnt: FCNT_DOWN,
        payload: Payload::Data { f_port: NonZeroU8::new(4).unwrap(), data: &[3, 2, 1] },
        ..Default::default()
    };
    frame.build_into(rx_buffer, &nwk, Some(&app)).unwrap().len()
}

#[tokio::test]
async fn failed_join_still_burns_a_durable_dev_nonce() {
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;

    let task = tokio::spawn(async move { device.join(&get_otaa_credentials()).await });
    timer.fire_most_recent().await; // RX1 open
    radio.handle_timeout().await; // RX1 timeout
    timer.fire_most_recent().await; // RX2 open
    radio.handle_timeout().await; // RX2 timeout
    assert!(matches!(task.await.unwrap(), Ok(JoinResponse::NoJoinAccept)));

    // DevNonce 0 went on the air and its increment was committed, despite no accept.
    let identity = store.identity();
    assert_eq!(identity.dev_nonce, 1);
    assert_eq!(identity.last_join_nonce, None);
    assert_eq!(identity.join_epoch, 0);
    // No session was written.
    assert!(store.blob(NvmRegion::Session).is_none());
}

#[tokio::test]
async fn dev_nonce_monotonic_across_power_loss() {
    const T: usize = 1;
    let store = MockStore::default();

    // First boot, first join: DevNonce 0.
    let (radio, timer, mut device) = restore(&store).await;
    let task = tokio::spawn(async move { device.join(&get_otaa_credentials()).await });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 1, 0>).await;
    assert!(matches!(task.await.unwrap(), Ok(JoinResponse::JoinSuccess)));
    let identity = store.identity();
    assert_eq!(
        (identity.dev_nonce, identity.last_join_nonce, identity.join_epoch),
        (1, Some(1), 1)
    );

    // Power loss. Next join must use DevNonce 1, not restart at 0 or draw randomly.
    let (radio, timer, mut device) = restore(&store).await;
    let task = tokio::spawn(async move { device.join(&get_otaa_credentials()).await });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 2, 1>).await;
    assert!(matches!(task.await.unwrap(), Ok(JoinResponse::JoinSuccess)));
    let identity = store.identity();
    assert_eq!(
        (identity.dev_nonce, identity.last_join_nonce, identity.join_epoch),
        (2, Some(2), 2)
    );
}

#[tokio::test]
async fn replayed_join_accept_rejected() {
    const T: usize = 2;
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;

    // Legitimate join with JoinNonce 5.
    let task = tokio::spawn(async move {
        let r = device.join(&get_otaa_credentials()).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 5, 0>).await;
    let (mut device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(JoinResponse::JoinSuccess)));
    let joined_epoch = store.identity().join_epoch;

    // An attacker replays a captured accept (JoinNonce 5 again): must be ignored,
    // ending in NoJoinAccept after both windows.
    let task = tokio::spawn(async move {
        let r = device.join(&get_otaa_credentials()).await;
        (device, r)
    });
    timer.fire_most_recent().await; // RX1 open

    // The replayed nonce is ignored; give the device time to reject the frame and arm
    // the RX2 timer.
    radio.handle_rxtx(join_accept::<T, 5, 1>).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
    timer.fire_most_recent().await; // RX2 open
    radio.handle_timeout().await; // RX2 timeout
    let (mut device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(JoinResponse::NoJoinAccept)));
    assert_eq!(store.identity().join_epoch, joined_epoch, "epoch must not advance on replay");

    // A fresh accept with a LOWER JoinNonce is accepted: 1.0.4 servers guarantee only
    // non-repetition, not monotonicity, so anything different from the last must pass.
    let task = tokio::spawn(async move {
        let r = device.join(&get_otaa_credentials()).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 3, 2>).await;
    let (mut device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(JoinResponse::JoinSuccess)));
    assert_eq!(store.identity().last_join_nonce, Some(3));

    // The replay floor tracks only the latest accept: nonce 5 is from two joins ago,
    // no longer the last accepted value, so it passes now.
    let task = tokio::spawn(async move { device.join(&get_otaa_credentials()).await });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 5, 3>).await;
    assert!(matches!(task.await.unwrap(), Ok(JoinResponse::JoinSuccess)));
    assert_eq!(store.identity().last_join_nonce, Some(5));
}

#[tokio::test]
async fn session_resumes_after_power_loss() {
    const T: usize = 3;
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;

    // Join and send one uplink (FCnt 0), no downlink.
    let task = tokio::spawn(async move {
        let r = device.join(&get_otaa_credentials()).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 1, 0>).await;
    let (mut device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(JoinResponse::JoinSuccess)));
    let keys_before = device.get_session().unwrap().get_session_keys().unwrap();

    let task = tokio::spawn(async move { device.send(&[1, 2, 3], 3, false).await });
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    assert!(matches!(task.await.unwrap(), Ok(SendResponse::RxComplete)));

    // Cold boot with total RAM loss.
    let (radio, timer, mut device) = restore(&store).await;
    let session = device.get_session().expect("session must resume");
    let keys_after = session.get_session_keys().unwrap();
    assert_eq!(format!("{keys_before:?}"), format!("{keys_after:?}"));
    // FCnt resumes from the persisted checkpoint (join set it to the margin), skipping
    // ahead of the single counter value actually used; never reusing one.
    assert_eq!(session.fcnt_up, crate::nvm::DEFAULT_FCNT_CHECKPOINT_MARGIN);

    // The resumed session can send immediately and accept a downlink.
    let task = tokio::spawn(async move {
        let r = device.send(&[1, 2, 3], 3, false).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_rxtx(data_downlink::<T, 32, 1>).await;
    let (_device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(SendResponse::DownlinkReceived(1))));
    // The advanced downlink counter (replay floor) was persisted at its exact value.
    assert_eq!(store.session().fcnt_down, Some(1));
}

#[tokio::test]
async fn stale_or_corrupt_session_dropped_identity_kept() {
    // Session blob from an older join epoch: identity survives, session does not.
    let store = MockStore::default();
    let identity = PersistentIdentity { dev_nonce: 7, last_join_nonce: Some(3), join_epoch: 5 };
    let mut buf = [0u8; MAX_BLOB_LEN];
    let n = identity.encode(&mut buf);
    store.put(NvmRegion::Identity, &buf[..n]);
    let stale = PersistentSession {
        nwkskey: crate::NwkSKey::from(get_key()),
        appskey: crate::AppSKey::from(get_key()),
        devaddr: crate::test_util::get_dev_addr(),
        fcnt_up_checkpoint: 100,
        fcnt_down: Some(9),
        join_epoch: 4, // does not match identity epoch 5
        data_rate: lorawan::types::DR::_0,
        rx1_delay: 1000,
        rx1_dr_offset: 0,
        rx2_data_rate: None,
        rx2_frequency: None,
        tx_power: None,
        adr_enabled: true,
    };
    let n = stale.encode(&mut buf);
    store.put(NvmRegion::Session, &buf[..n]);

    let (radio, timer, mut device) = restore(&store).await;
    assert!(device.get_session().is_none(), "stale-epoch session must not resume");

    // The kept identity still drives the join path: DevNonce continues at 7.
    let task = tokio::spawn(async move { device.join(&get_otaa_credentials()).await });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<4, 4, 7>).await;
    assert!(matches!(task.await.unwrap(), Ok(JoinResponse::JoinSuccess)));
    assert_eq!(store.identity().dev_nonce, 8);

    // Corrupt (torn) session blob: same outcome.
    let matching = PersistentSession { join_epoch: 6, ..stale };
    let n = matching.encode(&mut buf);
    store.put(NvmRegion::Session, &buf[..n]);
    let (_radio, _timer, mut device) = restore(&store).await;
    assert!(device.get_session().is_some(), "sanity: matching-epoch session resumes");
    store.corrupt(NvmRegion::Session);
    let (_radio, _timer, mut device) = restore(&store).await;
    assert!(device.get_session().is_none(), "corrupt session must not resume");
}

#[tokio::test]
async fn checkpoint_margin_gates_session_writes() {
    const T: usize = 5;
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;
    device.set_checkpoint_margin(2);

    let task = tokio::spawn(async move {
        let r = device.join(&get_otaa_credentials()).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 1, 0>).await;
    let (mut device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(JoinResponse::JoinSuccess)));
    let saves_after_join = store.saves(NvmRegion::Session);
    assert_eq!(store.session().fcnt_up_checkpoint, 2);

    // FCnt 0 and 1 are below the persisted checkpoint: no writes.
    for _ in 0..2 {
        let task = tokio::spawn(async move {
            let r = device.send(&[1, 2, 3], 3, false).await;
            (device, r)
        });
        timer.fire_most_recent().await;
        radio.handle_timeout().await;
        timer.fire_most_recent().await;
        radio.handle_timeout().await;
        let (d, response) = task.await.unwrap();
        assert!(matches!(response, Ok(SendResponse::RxComplete)));
        device = d;
    }
    assert_eq!(store.saves(NvmRegion::Session), saves_after_join);

    // FCnt 2 reaches the checkpoint: the blob is rewritten (before transmit) with the
    // next checkpoint at 2 + margin.
    let task = tokio::spawn(async move {
        let r = device.send(&[1, 2, 3], 3, false).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    let (_device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(SendResponse::RxComplete)));
    assert_eq!(store.saves(NvmRegion::Session), saves_after_join + 1);
    assert_eq!(store.session().fcnt_up_checkpoint, 4);
}

#[tokio::test]
async fn explicit_checkpoint_dedups_unchanged_state() {
    const T: usize = 6;
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;

    let task = tokio::spawn(async move {
        let r = device.join(&get_otaa_credentials()).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_rxtx(join_accept::<T, 1, 0>).await;
    let (mut device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(JoinResponse::JoinSuccess)));

    // First checkpoint moves the resume point from margin (32) to the live counter (0):
    // one session write, identity unchanged so no identity write.
    let identity_saves = store.saves(NvmRegion::Identity);
    let session_saves = store.saves(NvmRegion::Session);
    device.checkpoint().await.unwrap();
    assert_eq!(store.saves(NvmRegion::Session), session_saves + 1);
    assert_eq!(store.saves(NvmRegion::Identity), identity_saves);
    assert_eq!(store.session().fcnt_up_checkpoint, 0);

    // Second checkpoint with identical state: both writes deduped away.
    let session_saves = store.saves(NvmRegion::Session);
    device.checkpoint().await.unwrap();
    assert_eq!(store.saves(NvmRegion::Session), session_saves);
    assert_eq!(store.saves(NvmRegion::Identity), identity_saves);
}

/// Send an unconfirmed uplink with no downlink and hand the device back.
async fn send_no_downlink(
    radio: &super::radio::RadioChannel,
    timer: &super::timer::TimerChannel,
    mut device: Device,
) -> Device {
    let task = tokio::spawn(async move {
        let r = device.send(&[1, 2, 3], 3, false).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    let (device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(SendResponse::RxComplete)));
    device
}

#[tokio::test]
async fn abp_session_resumes_after_power_loss() {
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;
    assert!(matches!(device.join(&get_abp_credentials()).await, Ok(JoinResponse::JoinSuccess)));
    assert_eq!(store.session().fcnt_up_checkpoint, crate::nvm::DEFAULT_FCNT_CHECKPOINT_MARGIN);
    let device = send_no_downlink(&radio, &timer, device).await;
    drop(device);

    // Cold boot: the app activates ABP again with the same keys and the session
    // continues from the checkpoint instead of starting over at FCnt 0.
    let (radio, timer, mut device) = restore(&store).await;
    let session_saves = store.saves(NvmRegion::Session);
    assert!(matches!(device.join(&get_abp_credentials()).await, Ok(JoinResponse::JoinSuccess)));
    assert_eq!(store.saves(NvmRegion::Session), session_saves, "resume must not rewrite");
    assert_eq!(device.get_session().unwrap().fcnt_up, crate::nvm::DEFAULT_FCNT_CHECKPOINT_MARGIN);
    let _device = send_no_downlink(&radio, &timer, device).await;
    let mut uplink = radio.get_last_uplink().await;
    let Ok(PhyPayload::Data(data)) = parser::parse(uplink.data_mut()) else {
        panic!("not a data frame");
    };
    assert_eq!(data.fhdr().fcnt() as u32, crate::nvm::DEFAULT_FCNT_CHECKPOINT_MARGIN);
}

#[tokio::test]
async fn abp_rejoin_with_new_keys_starts_a_fresh_session() {
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;
    assert!(matches!(device.join(&get_abp_credentials()).await, Ok(JoinResponse::JoinSuccess)));
    let device = send_no_downlink(&radio, &timer, device).await;
    drop(device);

    // Different DevAddr: a new session at FCnt 0 with a fresh checkpoint, persisted.
    let (radio, timer, mut device) = restore(&store).await;
    device.set_checkpoint_margin(4);
    let new_keys = crate::JoinMode::ABP {
        devaddr: lorawan::parser::DevAddr::from_value(0x0102_0304),
        appskey: crate::AppSKey::from([9; 16]),
        nwkskey: crate::NwkSKey::from([8; 16]),
    };
    let session_saves = store.saves(NvmRegion::Session);
    assert!(matches!(device.join(&new_keys).await, Ok(JoinResponse::JoinSuccess)));
    assert_eq!(store.saves(NvmRegion::Session), session_saves + 1);
    let persisted = store.session();
    assert_eq!(persisted.devaddr, lorawan::parser::DevAddr::from_value(0x0102_0304));
    assert_eq!(persisted.fcnt_up_checkpoint, 4);
    assert_eq!(device.get_session().unwrap().fcnt_up, 0);
    let _device = send_no_downlink(&radio, &timer, device).await;
}

#[tokio::test]
async fn exhausted_dev_nonce_refuses_to_join() {
    let store = MockStore::default();
    let identity =
        PersistentIdentity { dev_nonce: u16::MAX, last_join_nonce: Some(3), join_epoch: 5 };
    let mut buf = [0u8; MAX_BLOB_LEN];
    let n = identity.encode(&mut buf);
    store.put(NvmRegion::Identity, &buf[..n]);

    let (_radio, _timer, mut device) = restore(&store).await;
    let response = device.join(&get_otaa_credentials()).await;
    assert!(matches!(
        response,
        Err(crate::async_device::Error::Mac(crate::mac::Error::DevNonceExhausted))
    ));
    // Nothing was written and nothing went on the air.
    assert_eq!(store.saves(NvmRegion::Identity), 0);
    assert_eq!(store.identity().dev_nonce, u16::MAX);
}

#[tokio::test]
async fn restore_validates_rx2_data_rate_and_keeps_adr() {
    let store = MockStore::default();
    let identity = PersistentIdentity { dev_nonce: 1, last_join_nonce: Some(1), join_epoch: 1 };
    let mut buf = [0u8; MAX_BLOB_LEN];
    let n = identity.encode(&mut buf);
    store.put(NvmRegion::Identity, &buf[..n]);
    let session = PersistentSession {
        nwkskey: crate::NwkSKey::from(get_key()),
        appskey: crate::AppSKey::from(get_key()),
        devaddr: crate::test_util::get_dev_addr(),
        fcnt_up_checkpoint: 10,
        fcnt_down: None,
        join_epoch: 1,
        data_rate: lorawan::types::DR::_2,
        rx1_delay: 1000,
        rx1_dr_offset: 0,
        // Not a US915 data rate: must not be adopted.
        rx2_data_rate: Some(lorawan::types::DR::_15),
        rx2_frequency: None,
        tx_power: None,
        adr_enabled: false,
    };
    let n = session.encode(&mut buf);
    store.put(NvmRegion::Session, &buf[..n]);

    let (_radio, _timer, device) = restore(&store).await;
    assert_eq!(device.mac.configuration.rx2_data_rate, None);
    assert_eq!(device.mac.configuration.data_rate, lorawan::types::DR::_2);
    assert!(!device.get_adr());
}

/// A certification-port downlink that yields a MAC response other than
/// `DownlinkReceived` still advances FCntDown, which must be persisted.
#[cfg(feature = "certification")]
#[tokio::test]
async fn certification_downlink_persists_fcnt_down() {
    let store = MockStore::default();
    let (radio, timer, mut device) = restore(&store).await;
    assert!(matches!(device.join(&get_abp_credentials()).await, Ok(JoinResponse::JoinSuccess)));
    assert_eq!(store.session().fcnt_down, None);

    fn cp_link_check_req(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        let frame = DataFrame {
            frame_type: DataFrameType::UnconfirmedDown,
            dev_addr: crate::test_util::get_dev_addr(),
            fcnt: 1,
            payload: Payload::Data { f_port: NonZeroU8::new(224).unwrap(), data: &[0x20] },
            ..Default::default()
        };
        frame.build_into(buf, &get_crypto(), Some(&get_crypto())).unwrap().len()
    }
    let task = tokio::spawn(async move {
        let r = device.send(&[1, 2, 3], 3, false).await;
        (device, r)
    });
    timer.fire_most_recent().await;
    radio.handle_rxtx(cp_link_check_req).await;
    let (_device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(SendResponse::RxComplete)));
    assert_eq!(store.session().fcnt_down, Some(1));
}
