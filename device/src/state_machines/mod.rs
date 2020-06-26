use super::*;

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
}

impl<R: radio::PhyRxTx + Timings> Shared<R> {

    pub fn get_mut_radio(&mut self) -> &mut R {
        &mut self.radio
    }
    pub fn get_mut_credentials(&mut self) -> &mut Credentials {
        &mut self.credentials
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
        }
    }
}
