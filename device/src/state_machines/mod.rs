use super::*;
use lorawan_encoding::parser::DecryptedDataPayload;

pub mod no_session;

pub mod session;

pub struct Shared<R: radio::PhyRxTx + Timings> {
    radio: R,
    credentials: Credentials,
    region: RegionalConfiguration,
    mac: Mac,
    // TODO: do something nicer for randomness
    get_random: fn() -> u32,
    buffer: Vec<u8, U256>,
    data_downlink: Option<DecryptedDataPayload<Vec<u8, U256>>>,
}

impl<R: radio::PhyRxTx + Timings> Shared<R> {
    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.radio
    }
    pub fn get_mut_credentials(&mut self) -> &mut Credentials {
        &mut self.credentials
    }

    pub fn take_data_downlink(&mut self) -> Option<DecryptedDataPayload<Vec<u8, U256>>> {
        self.data_downlink.take()
    }
}

impl<R: radio::PhyRxTx + Timings> Shared<R> {
    pub fn new(
        radio: R,
        credentials: Credentials,
        region: RegionalConfiguration,
        mac: Mac,
        get_random: fn() -> u32,
        buffer: Vec<u8, U256>,
    ) -> Shared<R> {
        Shared {
            radio,
            credentials,
            region,
            mac,
            get_random,
            buffer,
            data_downlink: None,
        }
    }
}

trait CommonState<R: radio::PhyRxTx + Timings> {
    fn get_mut_shared(&mut self) -> &mut Shared<R>;
}
