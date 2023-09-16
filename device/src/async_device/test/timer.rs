use crate::async_device::radio::Timer;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

impl TestTimer {
    pub fn new() -> (TimerChannel, Self) {
        let (tx, rx) = mpsc::channel(5);
        let armed_count = Arc::new(Mutex::new(0));
        (TimerChannel { tx, armed_count: armed_count.clone() }, Self { rx, armed_count })
    }
}

pub struct TestTimer {
    armed_count: Arc<Mutex<usize>>,
    rx: mpsc::Receiver<()>,
}

impl Timer for TestTimer {
    fn reset(&mut self) {}

    async fn at(&mut self, _millis: u64) {
        {
            *self.armed_count.lock().await += 1;
        }
        self.rx.recv().await;
    }

    async fn delay_ms(&mut self, _millis: u64) {
        {
            *self.armed_count.lock().await += 1;
        }
        self.rx.recv().await;
    }
}

/// A channel for the test fixture to trigger fires and to check calls.
pub struct TimerChannel {
    armed_count: Arc<Mutex<usize>>,
    tx: mpsc::Sender<()>,
}

impl TimerChannel {
    pub async fn fire(&self) {
        self.tx.send(()).await.unwrap();
    }

    pub async fn get_armed_count(&self) -> usize {
        *self.armed_count.lock().await
    }
}
