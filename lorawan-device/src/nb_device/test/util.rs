use super::*;
use crate::radio::{RfConfig, RxQuality};

use crate::nb_device::{
    Device, Timings,
    radio::{Event, PhyRxTx, Response},
};
use crate::test_util::*;
use region::{Configuration, Region};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub fn test_device() -> Device<TestRadio, rand_core::OsRng, 255> {
    Device::new(Configuration::new(Region::US915), TestRadio::default(), rand::rngs::OsRng)
}

#[derive(Clone)]
pub struct TestHandler {
    pub battery: u8,
    pub reboots: Arc<AtomicUsize>,
    pub last_device_time: Arc<Mutex<Option<crate::DeviceTime>>>,
}

impl TestHandler {
    pub fn with_battery(battery: u8) -> Self {
        Self {
            battery,
            reboots: Arc::new(AtomicUsize::new(0)),
            last_device_time: Arc::new(Mutex::new(None)),
        }
    }

    pub fn last_device_time(&self) -> Option<crate::DeviceTime> {
        *self.last_device_time.lock().unwrap()
    }
}

impl Default for TestHandler {
    fn default() -> Self {
        Self::with_battery(255)
    }
}

impl crate::DeviceHandler for TestHandler {
    fn battery_level(&self) -> u8 {
        self.battery
    }

    fn reboot(&mut self) {
        self.reboots.fetch_add(1, Ordering::SeqCst);
    }

    fn set_device_time(&mut self, time: crate::DeviceTime) {
        *self.last_device_time.lock().unwrap() = Some(time);
    }
}

pub fn test_device_with_handler<H: crate::DeviceHandler>(
    handler: H,
) -> Device<TestRadio, rand_core::OsRng, 255, 1, H> {
    Device::new_with_handler(
        handler,
        Configuration::new(Region::US915),
        TestRadio::default(),
        rand::rngs::OsRng,
    )
}

#[derive(Debug)]
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

    /// Take the most recently transmitted uplink, if any.
    pub fn take_last_uplink(&mut self) -> Option<Uplink> {
        self.last_uplink.take()
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

    const MAX_RADIO_POWER: u8 = 26;

    const ANTENNA_GAIN: i8 = 0;

    fn get_mut_radio(&mut self) -> &mut Self {
        self
    }
    fn get_received_packet(&mut self) -> &mut [u8] {
        &mut self.buffer[..self.buffer_index]
    }

    fn handle_event(&mut self, event: Event<'_, Self>) -> Result<Response<Self>, Self::PhyError>
    where
        Self: Sized,
    {
        match event {
            Event::TxRequest(config, buf) => {
                // ensure that we have always consumed the previous uplink
                if self.last_uplink.is_some() {
                    panic!("last uplink not consumed")
                }
                self.last_uplink =
                    Some(Uplink::new(buf, config).map_err(|_| "error creating uplink")?);
                return Ok(Response::TxDone(0));
            }
            Event::RxRequest(rf_config) => {
                self.current_config = Some(rf_config);
            }
            Event::CancelRx => (),
            Event::Phy(()) => {
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
