use crate::async_device::radio::Timer;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, Mutex};

impl TestTimer {
    pub fn new() -> (TimerChannel, Self) {
        let tx = Arc::new(Mutex::new(HashMap::new()));
        let armed_count = Arc::new(Mutex::new(0));
        (
            TimerChannel { tx: tx.clone(), armed_count: armed_count.clone() },
            Self { tx, armed_count },
        )
    }
}

pub struct TestTimer {
    armed_count: Arc<Mutex<usize>>,
    tx: Arc<Mutex<HashMap<usize, mpsc::Sender<()>>>>,
}

impl TestTimer {
    async fn create_channel_and_await(&mut self) {
        let (tx, mut rx) = mpsc::channel(1);
        {
            *self.armed_count.lock().await += 1;
            let mut tx_map = self.tx.lock().await;
            tx_map.insert(*self.armed_count.lock().await, tx);
        }
        rx.recv().await;
    }
}

impl Timer for TestTimer {
    fn reset(&mut self) {}

    async fn at(&mut self, _millis: u64) {
        self.create_channel_and_await().await;
    }

    async fn delay_ms(&mut self, _millis: u64) {
        self.create_channel_and_await().await;
    }
}

/// A channel for the test fixture to trigger fires and to check calls.
pub struct TimerChannel {
    armed_count: Arc<Mutex<usize>>,
    tx: Arc<Mutex<HashMap<usize, mpsc::Sender<()>>>>,
}

impl TimerChannel {
    pub async fn fire_most_recent(&self) {
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        let mut tx_map = self.tx.lock().await;
        let armed_count = *self.armed_count.lock().await;
        let tx = tx_map.remove(&armed_count).unwrap();
        tx.send(()).await.unwrap();
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
        *self.armed_count.lock().await
    }
}
