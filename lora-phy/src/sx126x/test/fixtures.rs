use crate::sx126x::{Config, Sx1261, Sx126x};
pub use crate::test_fixtures::{Delayer, DummyVariant, SpiError};
use embedded_hal::spi::Operation;
use embedded_hal_async::spi::SpiDevice;
use std::vec::Vec;

/// Records every SPI write so the byte stream of our driver can be compared
/// against the byte stream of Semtech's reference driver. Implements both the
/// blocking `SpiDevice` (for the reference driver, via smtc-modem-cores) and
/// the async one (for lora-phy).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestFixture {
    pub ops: Vec<Ops>,
}

impl TestFixture {
    pub fn new() -> Self {
        Self { ops: vec![] }
    }

    fn record(&mut self, operations: &mut [Operation<'_, u8>]) {
        let mut vec = Vec::new();
        for op in operations {
            match op {
                Operation::Write(buf) => vec.extend_from_slice(buf),
                _ => todo!(),
            }
        }
        self.ops.push(Ops::Write(vec));
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ops {
    Write(Vec<u8>),
}

impl embedded_hal::spi::ErrorType for TestFixture {
    type Error = SpiError;
}

impl embedded_hal::spi::SpiDevice for TestFixture {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.record(operations);
        Ok(())
    }
}

impl SpiDevice<u8> for TestFixture {
    async fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.ops.push(Ops::Write(buf.to_vec()));
        Ok(())
    }

    async fn read(&mut self, _buf: &mut [u8]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.record(operations);
        Ok(())
    }

    async fn transfer(&mut self, _read: &mut [u8], _write: &[u8]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transfer_in_place(&mut self, _buf: &mut [u8]) -> Result<(), Self::Error> {
        todo!()
    }
}

pub fn get_sx126x() -> Sx126x<TestFixture, DummyVariant, Sx1261> {
    Sx126x::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1261,
            tcxo_ctrl: None,
            use_dcdc: false,
            rx_boost: true,
        },
    )
}
