use super::*;
use lorawan_encoding::parser::DecryptedDataPayload;

pub mod no_session;
pub mod session;

pub use region::DR;

pub struct Shared<R: radio::PhyRxTx + Timings> {
    radio: R,
    credentials: Credentials,
    region: region::Configuration,
    mac: Mac,
    // TODO: do something nicer for randomness
    get_random: fn() -> u32,
    buffer: R::PhyBuf,
    downlink: Option<Downlink>,
    datarate: DR,
}

enum Downlink {
    Data(DecryptedDataPayload<Vec<u8, U256>>),
    Join(JoinAccept),
}

#[derive(Debug)]
pub struct JoinAccept {
    pub cflist: Option<[u32; 5]>,
}

impl<R: radio::PhyRxTx + Timings> Shared<R> {
    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.radio
    }
    pub fn get_mut_credentials(&mut self) -> &mut Credentials {
        &mut self.credentials
    }
    pub fn get_datarate(&mut self) -> DR {
        self.datarate
    }
    pub fn set_datarate(&mut self, datarate: DR) {
        self.datarate = datarate;
    }

    pub fn take_data_downlink(&mut self) -> Option<DecryptedDataPayload<Vec<u8, U256>>> {
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

impl<R: radio::PhyRxTx + Timings> Shared<R> {
    pub fn new(
        radio: R,
        credentials: Credentials,
        region: region::Configuration,
        mac: Mac,
        get_random: fn() -> u32,
        buffer: R::PhyBuf,
    ) -> Shared<R> {
        let datarate = region.get_default_datarate();
        Shared {
            radio,
            credentials,
            region,
            mac,
            get_random,
            buffer,
            downlink: None,
            datarate,
        }
    }
}

trait CommonState<R: radio::PhyRxTx + Timings> {
    fn get_mut_shared(&mut self) -> &mut Shared<R>;
}
