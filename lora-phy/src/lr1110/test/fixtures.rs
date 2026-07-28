use crate::lr1110::radio_kind_params::PaSelection;
use crate::lr1110::{Config, Lr1110};
pub use crate::test_fixtures::{Delayer, DummyVariant, SpiError};
use embedded_hal::spi::Operation;
use embedded_hal_async::spi::SpiDevice;
use std::collections::{HashMap, VecDeque};
use std::vec::Vec;

/// Records every SPI transaction so the byte stream of our driver can be
/// compared against the byte stream of Semtech's reference driver. Implements
/// both the blocking `SpiDevice` (for the reference driver, via
/// smtc-modem-cores) and the async one (for lora-phy).
///
/// Unlike the sx126x, the lr11xx read protocol is identical in both drivers —
/// a command-write transaction, then a separate read transaction clocking one
/// discarded status byte plus the data — so transactions compare exactly with
/// no wire canonicalization.
///
/// A read transaction carries no command bytes, so canned responses are keyed
/// by the most recent command write (`prime_read`). Command-less direct reads
/// (single-`Read` transactions) consume from a FIFO primed with
/// `prime_direct_read`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestFixture {
    pub ops: Vec<Ops>,
    read_responses: HashMap<Vec<u8>, Vec<u8>>,
    direct_read_responses: VecDeque<Vec<u8>>,
    last_cmd: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ops {
    Write(Vec<u8>),
    /// Total bytes clocked during a read transaction (MOSI idles at NOP)
    Read(usize),
}

impl TestFixture {
    pub fn new() -> Self {
        Self {
            ops: vec![],
            read_responses: HashMap::new(),
            direct_read_responses: VecDeque::new(),
            last_cmd: vec![],
        }
    }

    /// Canned response for the read phase of the command identified by its
    /// full command bytes. Covers the data only — the leading status byte
    /// both drivers discard always reads 0.
    pub fn prime_read(&mut self, command: &[u8], response: &[u8]) {
        self.read_responses.insert(command.to_vec(), response.to_vec());
    }

    /// Canned response for the next command-less direct read (all bytes,
    /// starting with Stat1)
    #[allow(dead_code)]
    pub fn prime_direct_read(&mut self, response: &[u8]) {
        self.direct_read_responses.push_back(response.to_vec());
    }

    fn record(&mut self, operations: &mut [Operation<'_, u8>]) {
        let mut cmd = Vec::new();
        let mut read_ops = 0;
        let mut read_len = 0;
        for op in operations.iter() {
            match op {
                Operation::Write(buf) => cmd.extend_from_slice(buf),
                Operation::Read(buf) => {
                    read_ops += 1;
                    read_len += buf.len();
                }
                _ => todo!(),
            }
        }
        if read_ops == 0 {
            self.last_cmd = cmd.clone();
            self.ops.push(Ops::Write(cmd));
            return;
        }
        // the lr11xx protocol never mixes writes and reads in one transaction
        assert!(cmd.is_empty(), "unexpected mixed write+read SPI transaction");

        // Two read ops = the response phase of a command (discarded status
        // byte + data); one read op = a direct read.
        let response = if read_ops == 1 {
            self.direct_read_responses.pop_front().unwrap_or_default()
        } else {
            let data = self.read_responses.get(&self.last_cmd).cloned().unwrap_or_default();
            // status byte first, then the data
            let mut full = vec![0u8];
            full.extend_from_slice(&data);
            full
        };
        let mut cursor = response.iter().copied();
        for op in operations.iter_mut() {
            if let Operation::Read(buf) = op {
                for b in buf.iter_mut() {
                    *b = cursor.next().unwrap_or(0);
                }
            }
        }
        self.ops.push(Ops::Read(read_len));
    }
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
        self.last_cmd = buf.to_vec();
        self.ops.push(Ops::Write(buf.to_vec()));
        Ok(())
    }

    async fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        let mut ops = [Operation::Read(buf)];
        self.record(&mut ops);
        Ok(())
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

/// HP-PA LR1110, no TCXO, no DC-DC, no RX boost — the plainest config so
/// individual commands compare 1:1
pub fn get_lr1110() -> Lr1110<TestFixture, DummyVariant> {
    Lr1110::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            pa_selection: PaSelection::Hp,
            dio_as_rf_switch: Some(Default::default()),
            tcxo_ctrl: None,
            use_dcdc: false,
            rx_boost: false,
        },
    )
}

/// LP-PA variant for the low-power TX path
pub fn get_lr1110_lp() -> Lr1110<TestFixture, DummyVariant> {
    Lr1110::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            pa_selection: PaSelection::Lp,
            dio_as_rf_switch: Some(Default::default()),
            tcxo_ctrl: None,
            use_dcdc: false,
            rx_boost: false,
        },
    )
}

/// TCXO + DC-DC board, for the init_system and TCXO-before-TX paths
pub fn get_lr1110_dcdc_tcxo() -> Lr1110<TestFixture, DummyVariant> {
    Lr1110::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            pa_selection: PaSelection::Hp,
            dio_as_rf_switch: Some(Default::default()),
            tcxo_ctrl: Some(crate::lr1110::TcxoCtrlVoltage::Ctrl1V8),
            use_dcdc: true,
            rx_boost: false,
        },
    )
}

/// RX-boosted config for the boosted do_rx path
pub fn get_lr1110_boosted() -> Lr1110<TestFixture, DummyVariant> {
    Lr1110::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            pa_selection: PaSelection::Hp,
            dio_as_rf_switch: Some(Default::default()),
            tcxo_ctrl: None,
            use_dcdc: false,
            rx_boost: true,
        },
    )
}
