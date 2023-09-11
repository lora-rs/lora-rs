use super::*;
use crate::async_device::radio::PhyRxTx;
use std::sync::Arc;
use std::vec::Vec;
use tokio::{
    sync::{mpsc, Mutex},
    time,
};
type RxTxHandler = fn(Option<Uplink>, RfConfig, &mut [u8]) -> usize;
impl TestRadio {
    pub fn new() -> (RadioChannel, Self) {
        let (tx, rx) = mpsc::channel(1);
        let last_uplink = Arc::new(Mutex::new(None));
        (RadioChannel { tx, last_uplink: last_uplink.clone() }, Self { rx, last_uplink })
    }
}

pub struct TestRadio {
    last_uplink: Arc<Mutex<Option<Uplink>>>,
    rx: mpsc::Receiver<RxTxHandler>,
}

pub struct Uplink {
    data: Vec<u8>,
    #[allow(unused)]
    tx_config: TxConfig,
}

impl Uplink {
    /// Creates a copy from a reference and ensures the packet is at least parseable.
    fn new(data_in: &[u8], tx_config: TxConfig) -> Result<Self, &'static str> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(data_in);
        let _parse = parse(data.as_mut_slice())?;
        Ok(Self { data, tx_config: tx_config })
    }

    pub fn get_payload(&mut self) -> PhyPayload<&mut [u8], DefaultFactory> {
        // unwrap since we verified parse in new
        parse(self.data.as_mut_slice()).unwrap()
    }
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
        *last_uplink = Some(Uplink::new(&buffer, config)?);
        Ok(length as u32)
    }

    async fn rx(
        &mut self,
        config: RfConfig,
        receiving_buffer: &mut [u8],
    ) -> Result<(usize, RxQuality), Self::PhyError> {
        let handler = self.rx.recv().await.unwrap();
        let mut last_uplink = self.last_uplink.lock().await;
        // a quick yield to let timer arm
        time::sleep(time::Duration::from_millis(50)).await;
        let size = handler(last_uplink.take(), config, receiving_buffer);
        Ok((size, RxQuality::new(0, 0)))
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
    tx: mpsc::Sender<RxTxHandler>,
}

impl RadioChannel {
    pub fn handle_rxtx(&self, handler: RxTxHandler) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(handler).await.unwrap();
        });
    }
}
