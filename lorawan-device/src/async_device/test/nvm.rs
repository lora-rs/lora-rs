//! Tests for the LoRaWAN 1.0.4 join rules: monotonic DevNonce with commit-before-transmit
//! and JoinNonce replay rejection. "Power loss" is simulated by dropping the device and
//! calling [`Device::restore`] against the same mock store.

use super::net::{ExchangeAction, Net, Pkt};
use super::radio::TestRadio;
use super::timer::TestTimer;
use super::{Device, JoinResponse, region, util};
use crate::nvm::{MAX_BLOB_LEN, NonVolatileStoreSync, NvmRegion, PersistentIdentity};
use crate::test_util::{Uplink, get_crypto};
use lorawan::parser::{self, JoinNonce, PhyPayload};
use std::convert::Infallible;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

/// A stored JoinNonce.
fn nonce(n: u32) -> Option<JoinNonce> {
    Some(JoinNonce::from_value(n))
}

#[derive(Default)]
struct Inner {
    blob: Option<Vec<u8>>,
    saves: usize,
}

/// In-memory store implementing the *sync* trait; the device consumes it through the
/// blanket async impl, so these tests cover that path too.
#[derive(Clone, Default)]
struct MockStore {
    inner: Arc<Mutex<Inner>>,
}

impl MockStore {
    fn saves(&self) -> usize {
        self.inner.lock().unwrap().saves
    }

    fn identity(&self) -> PersistentIdentity {
        PersistentIdentity::decode(self.inner.lock().unwrap().blob.as_ref().unwrap()).unwrap()
    }

    fn put(&self, identity: &PersistentIdentity) {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let n = identity.encode(&mut buf);
        self.inner.lock().unwrap().blob = Some(buf[..n].to_vec());
    }

    fn corrupt(&self) {
        let mut inner = self.inner.lock().unwrap();
        *inner.blob.as_mut().unwrap().last_mut().unwrap() ^= 0x01;
    }
}

impl NonVolatileStoreSync for MockStore {
    type Error = Infallible;

    fn save(&mut self, region: NvmRegion, bytes: &[u8]) -> Result<(), Infallible> {
        let NvmRegion::Identity = region;
        let mut inner = self.inner.lock().unwrap();
        inner.blob = Some(bytes.to_vec());
        inner.saves += 1;
        Ok(())
    }

    fn load(&mut self, region: NvmRegion, buf: &mut [u8]) -> Result<Option<usize>, Infallible> {
        let NvmRegion::Identity = region;
        Ok(self.inner.lock().unwrap().blob.as_ref().map(|blob| {
            buf[..blob.len()].copy_from_slice(blob);
            blob.len()
        }))
    }
}

/// Boot a device against `store`, as after a power cycle, wrapped in a network harness.
async fn restore(store: &MockStore) -> Net<MockStore> {
    let pending = Arc::new(AtomicUsize::new(0));
    let (radio, mock_radio) = TestRadio::new(pending.clone());
    let (timer, mock_timer) = TestTimer::new(pending);
    let region = region::US915::default();
    let device =
        Device::restore(region.into(), mock_radio, mock_timer, rand_core::OsRng, store.clone())
            .await
            .unwrap();
    Net::new(radio, timer, device).await
}

/// The DevNonce the device put in a JoinRequest.
fn dev_nonce(uplink: &Uplink) -> u16 {
    let mut uplink = uplink.clone();
    let Ok(PhyPayload::JoinRequest(join_request)) = parser::parse(uplink.data_mut()) else {
        panic!("not a join request");
    };
    assert!(join_request.validate_mic(&get_crypto()));
    join_request.dev_nonce().value()
}

/// Join with the network answering JoinNonce `nonce`; returns the DevNonce used.
async fn join(net: &mut Net<MockStore>, nonce: u8) -> u16 {
    let r = net.join(ExchangeAction::rx1(Pkt::join_accept().join_nonce(nonce))).await;
    r.expect_joined();
    dev_nonce(r.uplink(0))
}

#[tokio::test]
async fn failed_join_still_burns_a_durable_dev_nonce() {
    let store = MockStore::default();
    let mut net = restore(&store).await;

    let r = net.join(ExchangeAction::none()).await;
    r.expect_no_join_accept();
    assert_eq!(dev_nonce(r.uplink(0)), 0);

    // DevNonce 0 went on the air and its increment was committed, despite no accept.
    let identity = store.identity();
    assert_eq!(identity.dev_nonce, 1);
    assert_eq!(identity.last_join_nonce, None);
}

#[tokio::test]
async fn dev_nonce_monotonic_across_power_loss() {
    let store = MockStore::default();

    // First boot, first join: DevNonce 0.
    let mut net = restore(&store).await;
    assert_eq!(join(&mut net, 1).await, 0);
    let identity = store.identity();
    assert_eq!((identity.dev_nonce, identity.last_join_nonce), (1, nonce(1)));

    // Power loss. The next join must use DevNonce 1, not restart at 0 or draw randomly.
    drop(net);
    let mut net = restore(&store).await;
    assert_eq!(join(&mut net, 2).await, 1);
    let identity = store.identity();
    assert_eq!((identity.dev_nonce, identity.last_join_nonce), (2, nonce(2)));
}

#[tokio::test]
async fn stored_dev_nonce_drives_the_next_join() {
    let store = MockStore::default();
    store.put(&PersistentIdentity {
        dev_nonce: 7,
        last_join_nonce: nonce(3),
        ..Default::default()
    });

    let mut net = restore(&store).await;
    assert_eq!(join(&mut net, 4).await, 7);
    assert_eq!(store.identity().dev_nonce, 8);
}

#[tokio::test]
async fn corrupt_identity_restarts_the_dev_nonce() {
    // A torn identity write is treated as absent: the counter restarts at 0 and a join
    // server will reject the device until it passes the last value it saw. This is why
    // stores must write the identity atomically.
    let store = MockStore::default();
    store.put(&PersistentIdentity {
        dev_nonce: 7,
        last_join_nonce: nonce(3),
        ..Default::default()
    });
    store.corrupt();

    let mut net = restore(&store).await;
    assert_eq!(join(&mut net, 4).await, 0);
    assert_eq!(store.identity().dev_nonce, 1);
}

#[tokio::test]
async fn replayed_join_accept_rejected() {
    let store = MockStore::default();
    let mut net = restore(&store).await;

    // Legitimate join with JoinNonce 5.
    assert_eq!(join(&mut net, 5).await, 0);

    // An attacker replays a captured accept (JoinNonce 5 again): it is ignored, and the
    // join ends in NoJoinAccept after both windows.
    let r = net.join(ExchangeAction::rx1(Pkt::join_accept().join_nonce(5))).await;
    r.expect_no_join_accept();
    assert_eq!(r.windows(), 2);
    assert_eq!(dev_nonce(r.uplink(0)), 1);
    let identity = store.identity();
    assert_eq!((identity.dev_nonce, identity.last_join_nonce), (2, nonce(5)));

    // A fresh accept with a LOWER JoinNonce is accepted: 1.0.4 servers guarantee only
    // non-repetition, not monotonicity, so anything different from the last must pass.
    assert_eq!(join(&mut net, 3).await, 2);
    assert_eq!(store.identity().last_join_nonce, nonce(3));

    // The replay check remembers only the latest accept: nonce 5 is from two joins ago,
    // no longer the last accepted value, so it passes now.
    assert_eq!(join(&mut net, 5).await, 3);
    assert_eq!(store.identity().last_join_nonce, nonce(5));
}

#[tokio::test]
async fn exhausted_dev_nonce_refuses_to_join() {
    let store = MockStore::default();
    store.put(&PersistentIdentity {
        dev_nonce: u16::MAX,
        last_join_nonce: nonce(3),
        ..Default::default()
    });

    let mut net = restore(&store).await;
    let response = net.device().join(&crate::test_util::get_otaa_credentials()).await;
    assert!(matches!(
        response,
        Err(crate::async_device::Error::Mac(crate::mac::Error::DevNonceExhausted))
    ));
    // Nothing was written and nothing went on the air.
    assert_eq!(store.saves(), 0);
    assert_eq!(store.identity().dev_nonce, u16::MAX);
}




#[tokio::test]
async fn join_request_still_checks_out_without_a_store() {
    // Sanity for the harness change: a NoNvm device joins as before.
    let (radio, timer, device) = util::setup();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.join(ExchangeAction::rx1(Pkt::join_accept().join_nonce(9))).await;
    r.expect_joined();
    util::assert_join_request(r.uplink(0));
}
