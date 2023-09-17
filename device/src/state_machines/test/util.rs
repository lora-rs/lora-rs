use super::*;
use crate::radio::{Error, Event, Response, RfConfig, RxQuality};
use crate::test_util::*;
use crate::{radio::PhyRxTx, Device, Timings};
use lorawan::default_crypto;
use region::{Configuration, Region};

pub fn test_device(
    join_mode: JoinMode,
) -> Device<TestRadio, default_crypto::DefaultFactory, rand_core::OsRng, 255> {
    Device::new(
        Configuration::new(Region::US915),
        join_mode,
        TestRadio::default(),
        rand::rngs::OsRng,
    )
}

pub struct TestRadio {
    current_config: Option<RfConfig>,
    last_uplink: Option<Uplink>,
    rxtx_handler: Option<RxTxHandler>,
    buffer: [u8; 256],
    buffer_index: usize,
}

impl TestRadio {
    pub fn set_rxtx_handler(&mut self, handler: RxTxHandler) {
        self.rxtx_handler = Some(handler);
    }
}

impl Default for TestRadio {
    fn default() -> Self {
        Self {
            current_config: None,
            last_uplink: None,
            rxtx_handler: None,
            buffer: [0; 256],
            buffer_index: 0,
        }
    }
}

impl PhyRxTx for TestRadio {
    type PhyEvent = ();
    type PhyError = &'static str;
    type PhyResponse = ();

    fn get_mut_radio(&mut self) -> &mut Self {
        self
    }
    fn get_received_packet(&mut self) -> &mut [u8] {
        &mut self.buffer[..self.buffer_index]
    }

    fn handle_event(&mut self, event: Event<Self>) -> Result<Response<Self>, Error<Self::PhyError>>
    where
        Self: Sized,
    {
        match event {
            Event::TxRequest(config, buf) => {
                // ensure that we have always consumed the previous uplink
                if self.last_uplink.is_some() {
                    return Err(Error::PhyError("Radio already has an uplink"));
                }
                self.last_uplink = Some(Uplink::new(&buf, config).map_err(|e| Error::PhyError(e))?);
                return Ok(Response::TxDone(0));
            }
            Event::RxRequest(rf_config) => {
                self.current_config = Some(rf_config);
            }
            Event::CancelRx => (),
            Event::PhyEvent(()) => {
                if let (Some(rf_config), Some(rxtx_handler)) =
                    (self.current_config, self.rxtx_handler)
                {
                    self.buffer_index =
                        rxtx_handler(self.last_uplink.take(), rf_config, &mut self.buffer);
                    return Ok(Response::RxDone(RxQuality::new(0, 0)));
                }
            }
        }
        Ok(Response::Idle)
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
