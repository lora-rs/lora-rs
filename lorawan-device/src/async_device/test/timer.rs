use crate::async_device::radio::Timer;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, mpsc, watch};

/// The kind of the most recently armed timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKind {
    /// Armed by `Timer::at`: a receive window is about to open, so the
    /// device will next read a radio event.
    Window,
    /// Armed by `Timer::delay_ms`: a retransmission wait; the device will
    /// not read a radio event after this fire (it retransmits, or `send`
    /// returns).
    Delay,
}

impl TestTimer {
    pub fn new(pending: Arc<AtomicUsize>) -> (TimerChannel, Self) {
        let tx = Arc::new(Mutex::new(HashMap::new()));
        let (armed_count, armed_count_rx) = watch::channel(0);
        let kind = Arc::new(AtomicBool::new(true));
        (
            TimerChannel {
                tx: tx.clone(),
                armed_count: armed_count_rx,
                pending,
                kind: kind.clone(),
            },
            Self { tx, armed_count, kind },
        )
    }
}

pub struct TestTimer {
    armed_count: watch::Sender<usize>,
    kind: Arc<AtomicBool>,
    tx: Arc<Mutex<HashMap<usize, mpsc::Sender<()>>>>,
}

impl TestTimer {
    async fn create_channel_and_await(&mut self, kind: TimerKind) {
        self.kind.store(kind == TimerKind::Window, Ordering::SeqCst);
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut tx_map = self.tx.lock().await;
            // The map holds the timer before the count announces it.
            self.armed_count.send_modify(|count| {
                *count += 1;
                tx_map.insert(*count, tx);
            });
        }
        rx.recv().await;
    }
}

impl Timer for TestTimer {
    fn reset(&mut self) {}

    async fn at(&mut self, _millis: u64) {
        self.create_channel_and_await(TimerKind::Window).await;
    }

    async fn delay_ms(&mut self, _millis: u64) {
        self.create_channel_and_await(TimerKind::Delay).await;
    }
}

/// A channel for the test fixture to trigger fires and to check calls.
pub struct TimerChannel {
    armed_count: watch::Receiver<usize>,
    kind: Arc<AtomicBool>,
    pending: Arc<AtomicUsize>,
    tx: Arc<Mutex<HashMap<usize, mpsc::Sender<()>>>>,
}

impl TimerChannel {
    pub async fn fire_most_recent(&self) {
        // Don't let the timeout overtake an injected frame.
        let start = Instant::now();
        while self.pending.load(Ordering::SeqCst) != 0 {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "injected frame was not consumed before the timer fired"
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        let mut tx_map = self.tx.lock().await;
        let armed_count = *self.armed_count.borrow();
        let tx = tx_map.remove(&armed_count).unwrap();
        tx.send(()).await.unwrap();
    }

    /// Fire the most recent timer, first waiting until `count` timers have
    /// been armed. Use this when the device arms the timer as a result of an
    /// earlier test event (eg: a retransmission after an RX window) so the
    /// test does not race the device task.
    pub async fn fire_when_armed(&self, count: usize) {
        self.wait_armed(count).await;
        self.fire_most_recent().await;
    }

    /// Wait until at least `count` timers have been armed; returns the
    /// count at that point.
    pub async fn wait_armed(&self, count: usize) -> usize {
        let mut armed_count = self.armed_count.clone();
        *armed_count.wait_for(|c| *c >= count).await.expect("test timer dropped")
    }

    #[allow(unused)]
    pub async fn confirm_dropped_timer(&self, index: usize) {
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        let mut tx_map = self.tx.lock().await;
        let tx = tx_map.remove(&index).unwrap();
        if tx.try_send(()).is_ok() {
            panic!("Timer was not dropped");
        }
    }

    pub async fn get_armed_count(&self) -> usize {
        *self.armed_count.borrow()
    }

    /// The kind of the most recently armed timer (see [`TimerKind`]).
    pub fn last_timer_kind(&self) -> TimerKind {
        if self.kind.load(Ordering::SeqCst) {
            TimerKind::Window
        } else {
            TimerKind::Delay
        }
    }
}
