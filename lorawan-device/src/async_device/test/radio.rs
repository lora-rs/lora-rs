use super::*;
use crate::async_device::radio::{PhyRxTx, RxConfig, RxStatus};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    sync::{Mutex, mpsc},
    time,
};
impl TestRadio {
    pub fn new(pending: Arc<AtomicUsize>) -> (RadioChannel, Self) {
        let (tx, rx) = mpsc::channel(2);
        let last_rxconfig = Arc::new(Mutex::new(None));
        let last_uplink = Arc::new(Mutex::new(None));
        let uplinks = Arc::new(Mutex::new(std::vec::Vec::new()));
        (
            RadioChannel {
                tx,
                last_uplink: last_uplink.clone(),
                last_rxconfig: last_rxconfig.clone(),
                uplinks: uplinks.clone(),
                pending: pending.clone(),
            },
            Self { rx, last_rxconfig, last_uplink, uplinks, current_config: None, pending, snr: 0 },
        )
    }

    /// Copy a pre-built downlink frame into the rx buffer.
    fn inject(&self, frame: &[u8], rx_buf: &mut [u8]) -> (usize, RxQuality) {
        assert!(frame.len() <= rx_buf.len(), "injected frame does not fit the rx buffer");
        rx_buf[..frame.len()].copy_from_slice(frame);
        self.pending.fetch_sub(1, Ordering::SeqCst);
        (frame.len(), RxQuality::new(-80, self.snr))
    }

    pub fn set_snr(&mut self, snr: i8) {
        self.snr = snr
    }

    /// Return snr in a 6-bit scaled format as in DevStatusAns
    pub fn snr_scaled(&self) -> u8 {
        ((self.snr << 2) as u8) >> 2
    }
}

#[derive(Debug)]
enum Msg {
    RxTx(RxTxHandler),
    /// A pre-built downlink frame, delivered as-is.
    Frame(std::vec::Vec<u8>),
    Timeout,
    /// The radio reports a receive error, the way lora-phy's adapter does
    /// for a frame whose header or payload CRC failed.
    PhyError(&'static str),
}

pub struct TestRadio {
    current_config: Option<RxConfig>,
    last_rxconfig: Arc<Mutex<Option<RxConfig>>>,
    last_uplink: Arc<Mutex<Option<Uplink>>>,
    uplinks: Arc<Mutex<std::vec::Vec<Uplink>>>,
    rx: mpsc::Receiver<Msg>,
    pending: Arc<AtomicUsize>,
    snr: i8,
}

impl PhyRxTx for TestRadio {
    type PhyError = &'static str;

    const MAX_RADIO_POWER: u8 = 26;

    const ANTENNA_GAIN: i8 = 0;

    async fn tx(&mut self, config: TxConfig, buffer: &[u8]) -> Result<u32, Self::PhyError> {
        let length = buffer.len();
        let uplink = Uplink::new(buffer, config).map_err(|_| "Parse error")?;
        self.uplinks.lock().await.push(uplink.clone());
        // stash the uplink, to be consumed by channel or by rx handler
        let mut last_uplink = self.last_uplink.lock().await;
        *last_uplink = Some(uplink);
        Ok(length as u32)
    }

    async fn setup_rx(&mut self, config: RxConfig) -> Result<(), Self::PhyError> {
        self.current_config = Some(config);
        // Make current rx configuration available for test harness
        let mut last_rxconfig = self.last_rxconfig.lock().await;
        *last_rxconfig = Some(config);
        Ok(())
    }

    async fn rx_continuous(
        &mut self,
        rx_buf: &mut [u8],
    ) -> Result<(usize, RxQuality), Self::PhyError> {
        let msg = self.rx.recv().await.unwrap();
        match msg {
            Msg::RxTx(handler) => {
                let last_uplink = self.last_uplink.lock().await;
                // a quick yield to let timer arm
                time::sleep(time::Duration::from_millis(5)).await;
                if let Some(config) = &self.current_config {
                    let length = handler(last_uplink.clone(), config.rf, rx_buf);
                    self.pending.fetch_sub(1, Ordering::SeqCst);
                    Ok((length, RxQuality::new(-80, self.snr)))
                } else {
                    panic!("Trying to rx before settings config!")
                }
            }
            Msg::Frame(frame) => Ok(self.inject(&frame, rx_buf)),
            Msg::Timeout => Err("Unexpected Timeout"),
            Msg::PhyError(e) => Err(e),
        }
    }
    async fn rx_single(&mut self, rx_buf: &mut [u8]) -> Result<RxStatus, Self::PhyError> {
        let msg = self.rx.recv().await.unwrap();
        match msg {
            Msg::RxTx(handler) => {
                let last_uplink = self.last_uplink.lock().await;
                // a quick yield to let timer arm
                time::sleep(time::Duration::from_millis(5)).await;
                if let Some(config) = &self.current_config {
                    let length = handler(last_uplink.clone(), config.rf, rx_buf);
                    self.pending.fetch_sub(1, Ordering::SeqCst);
                    Ok(RxStatus::Rx(length, RxQuality::new(-80, self.snr)))
                } else {
                    panic!("Trying to rx before settings config!")
                }
            }
            Msg::Frame(frame) => {
                let (len, quality) = self.inject(&frame, rx_buf);
                Ok(RxStatus::Rx(len, quality))
            }
            Msg::Timeout => Ok(RxStatus::RxTimeout),
            Msg::PhyError(e) => Err(e),
        }
    }
}

impl Timings for TestRadio {
    fn get_rx_window_lead_time_ms(&self) -> u32 {
        10
    }
}

/// A channel for the test fixture to trigger fires and to check calls.
pub struct RadioChannel {
    #[allow(unused)]
    last_rxconfig: Arc<Mutex<Option<RxConfig>>>,
    last_uplink: Arc<Mutex<Option<Uplink>>>,
    uplinks: Arc<Mutex<std::vec::Vec<Uplink>>>,
    tx: mpsc::Sender<Msg>,
    pending: Arc<AtomicUsize>,
}

impl RadioChannel {
    pub async fn handle_rxtx(&self, handler: RxTxHandler) {
        tokio::time::sleep(time::Duration::from_millis(5)).await;
        // Keep the frame counted until its handler has run, so the timer waits
        // on this before firing.
        self.pending.fetch_add(1, Ordering::SeqCst);
        self.tx.send(Msg::RxTx(handler)).await.unwrap();
    }
    /// Deliver a pre-built downlink frame as-is (no handler).
    pub async fn handle_frame(&self, frame: std::vec::Vec<u8>) {
        tokio::time::sleep(time::Duration::from_millis(5)).await;
        self.pending.fetch_add(1, Ordering::SeqCst);
        self.tx.send(Msg::Frame(frame)).await.unwrap();
    }
    pub async fn handle_timeout(&self) {
        tokio::time::sleep(time::Duration::from_millis(5)).await;
        self.tx.send(Msg::Timeout).await.unwrap();
    }
    pub async fn handle_phy_error(&self, e: &'static str) {
        tokio::time::sleep(time::Duration::from_millis(5)).await;
        self.tx.send(Msg::PhyError(e)).await.unwrap();
    }

    pub async fn get_rxconfig(&self) -> Option<RxConfig> {
        let rxconf = self.last_rxconfig.lock().await;
        *rxconf
    }

    pub async fn get_last_uplink(&self) -> Uplink {
        let uplink = self.last_uplink.lock().await;
        uplink.clone().unwrap()
    }

    /// Take the uplinks transmitted since the last call, in order.
    pub async fn take_uplinks(&self) -> std::vec::Vec<Uplink> {
        self.uplinks.lock().await.drain(..).collect()
    }
}
