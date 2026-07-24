use crate::sx126x::{Config, Sx1261, Sx126x};
pub use crate::test_fixtures::{Delayer, DummyVariant, SpiError};
use embedded_hal::spi::Operation;
use embedded_hal_async::spi::SpiDevice;
use std::collections::HashMap;
use std::vec::Vec;

/// Records every SPI operation so the byte stream of our driver can be
/// compared against the byte stream of Semtech's reference driver. Implements
/// both the blocking `SpiDevice` (for the reference driver, via
/// smtc-modem-cores) and the async one (for lora-phy). Reads answer from
/// `read_responses` (zeros when unprimed) so read-modify-write command
/// sequences stay comparable across the two drivers.
#[derive(Clone, Debug)]
pub struct TestFixture {
    pub ops: Vec<Ops>,
    read_responses: HashMap<Vec<u8>, Vec<u8>>,
}

impl PartialEq for TestFixture {
    fn eq(&self, other: &Self) -> bool {
        self.ops == other.ops
    }
}
impl Eq for TestFixture {}

impl TestFixture {
    pub fn new() -> Self {
        Self {
            ops: vec![],
            read_responses: HashMap::new(),
        }
    }

    /// Canned response for a read identified by its full command bytes
    pub fn prime_read(&mut self, command: &[u8], response: &[u8]) {
        self.read_responses.insert(command.to_vec(), response.to_vec());
    }

    pub fn writes(&self) -> Vec<&Ops> {
        self.ops.iter().filter(|op| matches!(op, Ops::Write(_))).collect()
    }

    fn record(&mut self, operations: &mut [Operation<'_, u8>]) {
        let mut cmd = Vec::new();
        let mut read_len = 0;
        for op in operations.iter() {
            match op {
                Operation::Write(buf) => cmd.extend_from_slice(buf),
                Operation::Read(buf) => read_len += buf.len(),
                _ => todo!(),
            }
        }
        if read_len == 0 {
            self.ops.push(Ops::Write(cmd));
            return;
        }
        let response = self.read_responses.get(&cmd).cloned().unwrap_or_default();
        let mut cursor = response.iter().copied();
        for op in operations.iter_mut() {
            if let Operation::Read(buf) = op {
                for b in buf.iter_mut() {
                    *b = cursor.next().unwrap_or(0);
                }
            }
        }
        self.ops.push(Ops::Read(cmd, read_len));
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ops {
    Write(Vec<u8>),
    Read(Vec<u8>, usize),
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
