use super::*;
use crate::async_device::radio::{PhyRxTx, RxConfig, RxStatus};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, Mutex},
    time,
};
impl TestRadio {
    pub fn new() -> (RadioChannel, Self) {
        let (tx, rx) = mpsc::channel(2);
        let last_rxconfig = Arc::new(Mutex::new(None));
        let last_uplink = Arc::new(Mutex::new(None));
        (
            RadioChannel {
                tx,
                last_uplink: last_uplink.clone(),
                last_rxconfig: last_rxconfig.clone(),
            },
            Self { rx, last_rxconfig, last_uplink, current_config: None, snr: 0 },
        )
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
    Timeout,
}

pub struct TestRadio {
    current_config: Option<RxConfig>,
    last_rxconfig: Arc<Mutex<Option<RxConfig>>>,
    last_uplink: Arc<Mutex<Option<Uplink>>>,
    rx: mpsc::Receiver<Msg>,
    snr: i8,
}

impl PhyRxTx for TestRadio {
    type PhyError = &'static str;

    const MAX_RADIO_POWER: u8 = 26;

    const ANTENNA_GAIN: i8 = 0;

    async fn tx(&mut self, config: TxConfig, buffer: &[u8]) -> Result<u32, Self::PhyError> {
        let length = buffer.len();
        // stash the uplink, to be consumed by channel or by rx handler
        let mut last_uplink = self.last_uplink.lock().await;
        *last_uplink = Some(Uplink::new(buffer, config).map_err(|_| "Parse error")?);
        //Ok(length as u32)
        Ok(0)
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
                    Ok((length, RxQuality::new(-80, self.snr)))
                } else {
                    panic!("Trying to rx before settings config!")
                }
            }
            Msg::Timeout => Err("Unexpected Timeout"),
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
                    Ok(RxStatus::Rx(length, RxQuality::new(-80, self.snr)))
                } else {
                    panic!("Trying to rx before settings config!")
                }
            }
            Msg::Timeout => Ok(RxStatus::RxTimeout),
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
    tx: mpsc::Sender<Msg>,
}

impl RadioChannel {
    pub async fn handle_rxtx(&self, handler: RxTxHandler) {
        tokio::time::sleep(time::Duration::from_millis(5)).await;
        self.tx.send(Msg::RxTx(handler)).await.unwrap();
    }
    pub async fn handle_timeout(&self) {
        tokio::time::sleep(time::Duration::from_millis(5)).await;
        self.tx.send(Msg::Timeout).await.unwrap();
    }

    pub async fn get_rxconfig(&self) -> Option<RxConfig> {
        let rxconf = self.last_rxconfig.lock().await;
        *rxconf
    }

    pub async fn get_last_uplink(&self) -> Uplink {
        let uplink = self.last_uplink.lock().await;
        uplink.clone().unwrap()
    }
}
