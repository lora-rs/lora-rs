use super::radio::RadioBuffer;
use super::*;
use lorawan_encoding::parser::DecryptedDataPayload;

pub mod no_session;
pub mod session;

pub use region::DR;

pub struct Shared<'a, R: radio::PhyRxTx + Timings> {
    radio: R,
    credentials: Option<Credentials>,
    region: region::Configuration,
    mac: Mac,
    // TODO: do something nicer for randomness
    get_random: fn() -> u32,
    tx_buffer: RadioBuffer<'a>,
    downlink: Option<Downlink>,
    datarate: DR,
}

#[allow(clippy::large_enum_variant)]
enum Downlink {
    Data(DecryptedDataPayload<Vec<u8, 256>>),
    Join(JoinAccept),
}

#[derive(Debug)]
pub struct JoinAccept {
    pub cflist: Option<[u32; 5]>,
}

impl<'a, R: radio::PhyRxTx + Timings> Shared<'a, R> {
    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.radio
    }
    pub fn get_mut_credentials(&mut self) -> &mut Option<Credentials> {
        &mut self.credentials
    }
    pub fn get_datarate(&mut self) -> DR {
        self.datarate
    }
    pub fn set_datarate(&mut self, datarate: DR) {
        self.datarate = datarate;
    }

    pub fn take_data_downlink(&mut self) -> Option<DecryptedDataPayload<Vec<u8, 256>>> {
        if let Some(Downlink::Data(payload)) = self.downlink.take() {
            Some(payload)
        } else {
            None
        }
    }

    pub fn take_join_accept(&mut self) -> Option<JoinAccept> {
        if let Some(Downlink::Join(payload)) = self.downlink.take() {
            Some(payload)
        } else {
            None
        }
    }
}

impl<'a, R: radio::PhyRxTx + Timings> Shared<'a, R> {
    pub fn new(
        radio: R,
        credentials: Option<Credentials>,
        region: region::Configuration,
        mac: Mac,
        get_random: fn() -> u32,
        buffer: &'a mut [u8],
    ) -> Shared<R> {
        let datarate = region.get_default_datarate();
        Shared {
            radio,
            credentials,
            region,
            mac,
            get_random,
            tx_buffer: RadioBuffer::new(buffer),
            downlink: None,
            datarate,
        }
    }
}
