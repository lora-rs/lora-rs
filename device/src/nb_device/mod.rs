use super::radio::RadioBuffer;
use super::*;
use mac::Mac;

pub mod state;

pub use region::DR;

#[cfg(test)]
mod test;

pub struct Shared<R: radio::PhyRxTx + Timings, RNG: RngCore, const N: usize> {
    pub(crate) radio: R,
    pub(crate) rng: RNG,
    pub(crate) tx_buffer: RadioBuffer<N>,
    pub(crate) mac: Mac,
    pub(crate) downlink: Option<Downlink>,
}

impl<R: radio::PhyRxTx + Timings, RNG: RngCore, const N: usize> Shared<R, RNG, N> {
    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.radio
    }

    pub fn get_datarate(&mut self) -> DR {
        self.mac.configuration.tx_data_rate
    }
    pub fn set_datarate(&mut self, datarate: DR) {
        self.mac.configuration.tx_data_rate = datarate;
    }
}
