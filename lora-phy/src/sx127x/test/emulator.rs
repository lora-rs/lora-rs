//! A behavioral model of an sx127x chip with an *edge-triggered* DIO0 line.
//!
//! The register-file fixture compares final state, which is blind to IRQ
//! lifecycle bugs: RegIrqFlags is write-1-to-clear with no auto-clear on
//! mode changes (SX1276 DS §4.1.2.4), and the MCU's DIO0 interrupt is
//! edge-triggered, so a stale flag holds the line high and a level-based
//! model would wave the bug through. Here `await_irq` completes only on a
//! 0→1 transition of the DIO0 level derived from RegIrqFlags, the IRQ mask
//! and the DIO0 mapping — a stale flag means no edge, exactly like real
//! hardware.
//!
//! The chip "runs" inside `await_irq`: in-flight operations complete (and
//! raise their IRQs) only while the host is waiting on the line, so every
//! completion produces an observable edge or an observable hang.
use super::fixtures::RESET_VALUES;
use crate::mod_params::RadioError;
use crate::mod_traits::InterfaceVariant;
use crate::sx127x::{Config, Sx127x, Sx1276};
use crate::test_fixtures::SpiError;
use embedded_hal::spi::Operation;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::SpiDevice;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::vec::Vec;

const REG_FIFO: u8 = 0x00;
const REG_OP_MODE: u8 = 0x01;
const REG_FIFO_ADDR_PTR: u8 = 0x0D;
const REG_FIFO_TX_BASE: u8 = 0x0E;
const REG_FIFO_RX_BASE: u8 = 0x0F;
const REG_FIFO_RX_CURRENT: u8 = 0x10;
const REG_IRQ_FLAGS_MASK: u8 = 0x11;
const REG_IRQ_FLAGS: u8 = 0x12;
const REG_RX_NB_BYTES: u8 = 0x13;
const REG_PKT_SNR: u8 = 0x19;
const REG_PKT_RSSI: u8 = 0x1A;
const REG_PAYLOAD_LENGTH: u8 = 0x22;
const REG_DIO_MAPPING1: u8 = 0x40;

const IRQ_RX_DONE: u8 = 0x40;
const IRQ_TX_DONE: u8 = 0x08;
const IRQ_CAD_DONE: u8 = 0x04;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Sleep,
    Standby,
    Tx,
    Rx,
    Cad,
    Other,
}

pub struct PendingRx {
    pub payload: Vec<u8>,
    pub rssi_raw: u8,
    pub snr_db: i8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TxRecord {
    pub frequency_raw: u32,
    pub payload: Vec<u8>,
}

pub struct ChipModel {
    regs: HashMap<u8, u8>,
    fifo: [u8; 256],
    tx_in_flight: bool,
    pending_rx: Option<PendingRx>,
    pub tx_log: Vec<TxRecord>,
}

impl ChipModel {
    fn new() -> Self {
        Self {
            regs: RESET_VALUES.iter().copied().collect(),
            fifo: [0; 256],
            tx_in_flight: false,
            pending_rx: None,
            tx_log: vec![],
        }
    }

    fn reg(&self, address: u8) -> u8 {
        *self.regs.get(&address).unwrap_or(&0)
    }

    pub fn mode(&self) -> Mode {
        match self.reg(REG_OP_MODE) & 0x07 {
            0 => Mode::Sleep,
            1 => Mode::Standby,
            3 => Mode::Tx,
            5 | 6 => Mode::Rx,
            7 => Mode::Cad,
            _ => Mode::Other,
        }
    }

    /// Latch an IRQ through the mask register (masked flags never appear)
    fn raise_irq(&mut self, irq: u8) {
        let flags = self.reg(REG_IRQ_FLAGS) | (irq & !self.reg(REG_IRQ_FLAGS_MASK));
        self.regs.insert(REG_IRQ_FLAGS, flags);
    }

    /// The DIO0 pin level: the flag currently routed to DIO0 by
    /// RegDioMapping1 bits 7:6 (00 RxDone, 01 TxDone, 10 CadDone)
    fn dio0_level(&self) -> bool {
        let flag = match self.reg(REG_DIO_MAPPING1) >> 6 {
            0 => IRQ_RX_DONE,
            1 => IRQ_TX_DONE,
            2 => IRQ_CAD_DONE,
            _ => return false,
        };
        self.reg(REG_IRQ_FLAGS) & flag != 0
    }

    fn deliver_rx(&mut self, rx: PendingRx) {
        let base = self.reg(REG_FIFO_RX_BASE);
        for (i, b) in rx.payload.iter().enumerate() {
            self.fifo[base.wrapping_add(i as u8) as usize] = *b;
        }
        self.regs.insert(REG_RX_NB_BYTES, rx.payload.len() as u8);
        self.regs.insert(REG_FIFO_RX_CURRENT, base);
        self.regs.insert(REG_PKT_SNR, (rx.snr_db as i16 * 4) as u8);
        self.regs.insert(REG_PKT_RSSI, rx.rssi_raw);
        self.raise_irq(IRQ_RX_DONE);
    }

    /// Advance chip time: complete whatever operation is in flight. Runs
    /// only while the host waits on the IRQ line, so completions are
    /// observable as edges (or observably missing).
    fn step(&mut self) {
        if self.tx_in_flight {
            self.tx_in_flight = false;
            let base = self.reg(REG_FIFO_TX_BASE);
            let len = self.reg(REG_PAYLOAD_LENGTH);
            let payload = (0..len).map(|i| self.fifo[base.wrapping_add(i) as usize]).collect();
            let frequency_raw =
                ((self.reg(0x06) as u32) << 16) | ((self.reg(0x07) as u32) << 8) | self.reg(0x08) as u32;
            self.tx_log.push(TxRecord { frequency_raw, payload });
            // TX auto-returns to standby
            let op_mode = (self.reg(REG_OP_MODE) & !0x07) | 0x01;
            self.regs.insert(REG_OP_MODE, op_mode);
            self.raise_irq(IRQ_TX_DONE);
        } else if self.mode() == Mode::Rx
            && let Some(rx) = self.pending_rx.take()
        {
            self.deliver_rx(rx);
        }
    }

    fn write(&mut self, address: u8, value: u8) {
        match address {
            REG_FIFO => {
                let ptr = self.reg(REG_FIFO_ADDR_PTR);
                self.fifo[ptr as usize] = value;
                self.regs.insert(REG_FIFO_ADDR_PTR, ptr.wrapping_add(1));
            }
            REG_IRQ_FLAGS => {
                // write-1-to-clear
                let flags = self.reg(REG_IRQ_FLAGS) & !value;
                self.regs.insert(REG_IRQ_FLAGS, flags);
            }
            REG_OP_MODE => {
                self.regs.insert(REG_OP_MODE, value);
                if self.mode() == Mode::Tx {
                    self.tx_in_flight = true;
                }
            }
            _ => {
                self.regs.insert(address, value);
            }
        }
    }

    fn read(&mut self, address: u8) -> u8 {
        if address == REG_FIFO {
            let ptr = self.reg(REG_FIFO_ADDR_PTR);
            self.regs.insert(REG_FIFO_ADDR_PTR, ptr.wrapping_add(1));
            self.fifo[ptr as usize]
        } else {
            self.reg(address)
        }
    }
}

/// Test handle to the chip shared by the fake SPI bus and IRQ lines.
#[derive(Clone)]
pub struct Chip(Arc<Mutex<ChipModel>>);

impl Chip {
    pub fn new() -> Self {
        Chip(Arc::new(Mutex::new(ChipModel::new())))
    }

    /// A packet arrives over the air: delivered immediately if the chip is
    /// receiving, otherwise held until it next enters RX (the chip acts on
    /// its own clock — no SPI traffic is involved).
    pub fn inject_rx(&self, payload: &[u8], rssi_raw: u8, snr_db: i8) {
        let rx = PendingRx {
            payload: payload.to_vec(),
            rssi_raw,
            snr_db,
        };
        self.with_model(|m| {
            if m.mode() == Mode::Rx {
                m.deliver_rx(rx);
            } else {
                m.pending_rx = Some(rx);
            }
        })
    }

    pub fn with_model<R>(&self, f: impl FnOnce(&mut ChipModel) -> R) -> R {
        f(&mut self.0.lock().unwrap())
    }
}

/// SPI bus wired to the chip model. sx127x transactions are an address byte
/// (wnr in bit 7) followed by data bytes; bursts auto-increment except at
/// the FIFO port, which the model handles in read/write.
pub struct FakeSpi(pub Chip);

impl embedded_hal::spi::ErrorType for FakeSpi {
    type Error = SpiError;
}

impl SpiDevice<u8> for FakeSpi {
    async fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let mut cmd: Vec<u8> = Vec::new();
        for op in operations.iter() {
            if let Operation::Write(buf) = op {
                cmd.extend_from_slice(buf);
            }
        }
        assert!(!cmd.is_empty(), "sx127x transaction with no address byte");
        let wnr = cmd[0] & 0x80 != 0;
        let address = cmd[0] & 0x7F;
        self.0.with_model(|m| {
            if wnr {
                for (i, b) in cmd[1..].iter().enumerate() {
                    let addr = if address == REG_FIFO {
                        address
                    } else {
                        address + i as u8
                    };
                    m.write(addr, *b);
                }
            } else {
                let mut offset = 0u8;
                for op in operations.iter_mut() {
                    if let Operation::Read(buf) = op {
                        for b in buf.iter_mut() {
                            let addr = if address == REG_FIFO { address } else { address + offset };
                            *b = m.read(addr);
                            offset += 1;
                        }
                    }
                }
            }
        });
        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.transaction(&mut [Operation::Write(buf)]).await
    }

    async fn read(&mut self, _buf: &mut [u8]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transfer(&mut self, _read: &mut [u8], _write: &[u8]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transfer_in_place(&mut self, _buf: &mut [u8]) -> Result<(), Self::Error> {
        todo!()
    }
}

/// IRQ/reset lines wired to the same chip model. `await_irq` is
/// edge-triggered like a real MCU GPIO interrupt: it samples DIO0 when the
/// wait is armed and completes only on a low→high transition. A flag
/// already latched at arm time produces no edge — ever — which is the
/// hardware failure mode stale-IRQ bugs cause.
pub struct FakeIv(pub Chip);

impl InterfaceVariant for FakeIv {
    async fn reset(&mut self, _delay: &mut impl DelayNs) -> Result<(), RadioError> {
        self.0.with_model(|m| *m = ChipModel::new());
        Ok(())
    }
    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
    async fn await_irq(&mut self) -> Result<(), RadioError> {
        let mut prev = self.0.with_model(|m| m.dio0_level());
        for _ in 0..1000 {
            let level = self.0.with_model(|m| {
                m.step();
                m.dio0_level()
            });
            if level && !prev {
                return Ok(());
            }
            prev = level;
            tokio::task::yield_now().await;
        }
        // No edge and nothing left that could produce one
        Err(RadioError::Irq)
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

pub fn get_emulated_sx1276(chip: &Chip) -> Sx127x<FakeSpi, FakeIv, Sx1276> {
    Sx127x::new(
        FakeSpi(chip.clone()),
        FakeIv(chip.clone()),
        Config {
            chip: Sx1276,
            tcxo_used: false,
            tx_boost: false,
            rx_boost: false,
        },
    )
}
