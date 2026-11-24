//! Scaffolding shared by the per-chip comparison-test fixtures: zero-semantics
//! boilerplate plus the command-stream SPI recorder used by the chips that
//! share the sx126x command protocol. Register-file chips (sx127x) and the
//! lr11xx keep their own recorders, their equality rules differ by design.
use crate::mod_params::RadioError;
use crate::mod_traits::InterfaceVariant;
use embedded_hal::spi::{ErrorKind, Operation};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::SpiDevice;
use std::collections::{HashMap, VecDeque};
use std::vec::Vec;

/// SPI error type for fixtures that never fail
#[derive(Debug)]
pub enum SpiError {}
impl embedded_hal::spi::Error for SpiError {
    fn kind(&self) -> ErrorKind {
        match *self {}
    }
}

/// Delay provider that returns immediately
pub struct Delayer;
impl DelayNs for Delayer {
    async fn delay_ns(&mut self, _ns: u32) {}
}

/// InterfaceVariant with no-op control lines (busy always ready, IRQ
/// always pending, RF switch ignored)
pub struct DummyVariant;

impl InterfaceVariant for DummyVariant {
    async fn reset(&mut self, _delay: &mut impl DelayNs) -> Result<(), RadioError> {
        Ok(())
    }
    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
    async fn await_irq(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
    async fn disable_rf_switch(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
}

/// Records every SPI operation so the byte stream of our driver can be
/// compared against the byte stream of Semtech's reference driver, for the
/// command-based chips (sx126x, sx128x) whose reads are `[opcode, NOP..]`
/// followed by the response bytes. Implements
/// both the blocking `SpiDevice` (for the reference driver, via
/// smtc-modem-cores) and the async one (for lora-phy). Reads answer from
/// `read_responses` (zeros when unprimed) so read-modify-write command
/// sequences stay comparable across the two drivers.
#[derive(Clone, Debug)]
pub struct CommandStreamFixture {
    pub ops: Vec<Ops>,
    read_responses: HashMap<Vec<u8>, VecDeque<Vec<u8>>>,
}

/// Wire-canonical form of one SPI transaction: (written bytes with trailing
/// NOPs trimmed, total bytes clocked). The two drivers split transactions
/// differently — C sends `[opcode, NOP]` then reads N, ours sends `[opcode]`
/// then reads status + N — but on the wire both clock the same bytes: MOSI
/// idles at 0x00 during reads, so a trailing NOP write and a read byte are
/// indistinguishable to the chip.
fn canonical(ops: &[Ops]) -> Vec<(&[u8], usize)> {
    ops.iter()
        .map(|op| {
            let (cmd, total) = match op {
                Ops::Write(cmd) => (cmd, cmd.len()),
                Ops::Read(cmd, read_len) => (cmd, cmd.len() + read_len),
            };
            let trimmed = cmd.len() - cmd.iter().rev().take_while(|b| **b == 0).count();
            (&cmd[..trimmed], total)
        })
        .collect()
}

impl PartialEq for CommandStreamFixture {
    fn eq(&self, other: &Self) -> bool {
        canonical(&self.ops) == canonical(&other.ops)
    }
}
impl Eq for CommandStreamFixture {}

impl CommandStreamFixture {
    pub fn new() -> Self {
        Self {
            ops: vec![],
            read_responses: HashMap::new(),
        }
    }

    /// Canned response for a read identified by its full command bytes.
    /// Priming the same command again queues a response for the next read
    /// (read-modify-write sequences hit the same command repeatedly).
    pub fn prime_read(&mut self, command: &[u8], response: &[u8]) {
        self.read_responses
            .entry(command.to_vec())
            .or_default()
            .push_back(response.to_vec());
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
        let response = self
            .read_responses
            .get_mut(&cmd)
            .and_then(|queue| queue.pop_front())
            .unwrap_or_default();
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

impl embedded_hal::spi::ErrorType for CommandStreamFixture {
    type Error = SpiError;
}

impl embedded_hal::spi::SpiDevice for CommandStreamFixture {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.record(operations);
        Ok(())
    }
}

impl SpiDevice<u8> for CommandStreamFixture {
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
