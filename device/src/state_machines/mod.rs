use super::radio::RadioBuffer;
use super::*;
use lorawan::parser::DecryptedDataPayload;

pub mod no_session;
pub mod session;

pub use region::DR;

#[cfg(test)]
mod test;

pub struct Shared<R: radio::PhyRxTx + Timings, RNG: RngCore, const N: usize> {
    radio: R,
    credentials: Option<Credentials>,
    region: region::Configuration,
    mac: Mac,
    // TODO: do something nicer for randomness
    rng: RNG,
    tx_buffer: RadioBuffer<N>,
    downlink: Option<Downlink>,
    datarate: DR,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum Downlink {
    Data(DecryptedDataPayload<Vec<u8, 256>>),
    Join,
}

impl<R: radio::PhyRxTx + Timings, RNG: RngCore, const N: usize> Shared<R, RNG, N> {
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
        if let Some(Downlink::Data(payload)) = self.mac.downlink.take() {
            Some(payload)
        } else {
            None
        }
    }
}

impl<R: radio::PhyRxTx + Timings, RNG: RngCore, const N: usize> Shared<R, RNG, N> {
    pub fn new(
        radio: R,
        credentials: Option<Credentials>,
        region: region::Configuration,
        mac: Mac,
        rng: RNG,
    ) -> Shared<R, RNG, N> {
        let datarate = region.get_default_datarate();
        Shared {
            radio,
            credentials,
            region,
            mac,
            rng,
            tx_buffer: RadioBuffer::new(),
            downlink: None,
            datarate,
        }
    }
}
