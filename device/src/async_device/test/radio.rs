use super::*;
use crate::async_device::radio::PhyRxTx;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, Mutex},
    time,
};
impl TestRadio {
    pub fn new() -> (RadioChannel, Self) {
        let (tx, rx) = mpsc::channel(2);
        let last_uplink = Arc::new(Mutex::new(None));
        (
            RadioChannel { tx, last_uplink: last_uplink.clone() },
            Self { rx, last_uplink, current_config: None },
        )
    }
}

enum Msg {
    RxTx(RxTxHandler),
    Preamble,
}

pub struct TestRadio {
    current_config: Option<RfConfig>,
    last_uplink: Arc<Mutex<Option<Uplink>>>,
    rx: mpsc::Receiver<Msg>,
}

impl PhyRxTx for TestRadio {
    type PhyError = &'static str;

    async fn tx(&mut self, config: TxConfig, buffer: &[u8]) -> Result<u32, Self::PhyError> {
        let length = buffer.len();
        // stash the uplink, to be consumed by channel or by rx handler
        let mut last_uplink = self.last_uplink.lock().await;
        // ensure that we have always consumed the previous uplink
        if last_uplink.is_some() {
            return Err("Radio already has an uplink");
        }
        *last_uplink = Some(Uplink::new(buffer, config)?);
        Ok(length as u32)
    }

    async fn setup_rx(&mut self, config: RfConfig) -> Result<(), Self::PhyError> {
        self.current_config = Some(config);
        Ok(())
    }

    async fn rx_until_state(
        &mut self,
        rx_buf: &mut [u8],
        target_state: TargetRxState,
    ) -> Result<RxState, Self::PhyError> {
        let msg = self.rx.recv().await.unwrap();
        match (msg, target_state) {
            (Msg::Preamble, TargetRxState::PreambleReceived) => {
                Ok(RxState::PreambleReceived)
            }
            (Msg::Preamble, TargetRxState::Done) => {
                panic!("Received preamble when TargetRxState::Done")
            }
            (Msg::RxTx(handler), TargetRxState::Done) => {
                let mut last_uplink = self.last_uplink.lock().await;
                // a quick yield to let timer arm
                time::sleep(time::Duration::from_millis(5)).await;
                if let Some(config) = &self.current_config {
                    let length = handler(last_uplink.take(), *config, rx_buf);
                    Ok(RxState::Done { length: length as u8, lq: RxQuality::new(-80, 0) })
                } else {
                    panic!("Trying to rx before settings config!")
                }
            }
            (Msg::RxTx(_), TargetRxState::PreambleReceived) => {
                panic!("Sent handler before sending preamble")
            }
        }
    }
}

impl Timings for TestRadio {
    fn get_rx_window_offset_ms(&self) -> i32 {
        0
    }
    fn get_rx_window_duration_ms(&self) -> u32 {
        100
    }
}

/// A channel for the test fixture to trigger fires and to check calls.
pub struct RadioChannel {
    #[allow(unused)]
    last_uplink: Arc<Mutex<Option<Uplink>>>,
    tx: mpsc::Sender<Msg>,
}

impl RadioChannel {
    pub fn handle_rxtx(&self, handler: RxTxHandler) {
        self.fire_preamble();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(Msg::RxTx(handler)).await.unwrap();
        });
    }

    pub fn fire_preamble(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(Msg::Preamble).await.unwrap();
        });
    }
}
