//! A behavioral model of an sx126x chip driven over SPI. The real `Sx126x`
//! driver and `LoRa` state machine run unmodified on top; tests hold a
//! `Chip` handle to inject received packets and inspect transmitted ones.
use crate::mod_params::RadioError;
use crate::mod_traits::InterfaceVariant;
use crate::sx126x::radio_kind_params::{IrqMask, OpCode};
use crate::sx126x::{Config, Sx1261, Sx126x};
use crate::test_fixtures::SpiError;
use embedded_hal::spi::Operation;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::SpiDevice;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::vec::Vec;

// Dispatch constants derived from the driver's enums (enum variants can't
// be `match` patterns against a raw byte, consts can). The byte values
// themselves are pinned independently by the SWL2001 comparison suite, so
// deriving from the driver here is not circular — the emulator's job is
// behavior, not encoding.
const WRITE_REGISTER: u8 = OpCode::WriteRegister as u8;
const READ_REGISTER: u8 = OpCode::ReadRegister as u8;
const WRITE_BUFFER: u8 = OpCode::WriteBuffer as u8;
const READ_BUFFER: u8 = OpCode::ReadBuffer as u8;
const SET_SLEEP: u8 = OpCode::SetSleep as u8;
const SET_STANDBY: u8 = OpCode::SetStandby as u8;
const SET_TX: u8 = OpCode::SetTx as u8;
const SET_RX: u8 = OpCode::SetRx as u8;
const SET_RF_FREQUENCY: u8 = OpCode::SetRFFrequency as u8;
const SET_PACKET_PARAMS: u8 = OpCode::SetPacketParams as u8;
const SET_BUFFER_BASE_ADDRESS: u8 = OpCode::SetBufferBaseAddress as u8;
const CFG_DIO_IRQ: u8 = OpCode::CfgDIOIrq as u8;
const GET_IRQ_STATUS: u8 = OpCode::GetIrqStatus as u8;
const CLR_IRQ_STATUS: u8 = OpCode::ClrIrqStatus as u8;
const GET_RX_BUFFER_STATUS: u8 = OpCode::GetRxBufferStatus as u8;
const GET_PACKET_STATUS: u8 = OpCode::GetPacketStatus as u8;
const SET_CAD: u8 = OpCode::SetCAD as u8;

const IRQ_TX_DONE: u16 = IrqMask::TxDone as u16;
const IRQ_RX_DONE: u16 = IrqMask::RxDone as u16;
const IRQ_CAD_DONE: u16 = IrqMask::CADDone as u16;
const IRQ_CAD_DETECTED: u16 = IrqMask::CADActivityDetected as u16;
const IRQ_RX_TX_TIMEOUT: u16 = IrqMask::RxTxTimeout as u16;

// No Tx variant: the model completes transmissions instantly inside SET_TX,
// so the chip is never observable mid-transmit
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Standby,
    Sleep,
    Rx,
}

pub struct PendingRx {
    pub payload: Vec<u8>,
    pub rssi_dbm: i16,
    pub snr_db: i16,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TxRecord {
    pub frequency_raw: u32,
    pub payload: Vec<u8>,
}

pub struct ChipModel {
    pub mode: Mode,
    pub registers: HashMap<u16, u8>,
    pub buffer: [u8; 256],
    tx_base_addr: u8,
    payload_length: u8,
    frequency_raw: u32,
    irq_status: u16,
    irq_mask: u16,
    dio1_mask: u16,
    rx_len: u8,
    pub cad_activity: bool,
    pub pending_rx: Option<PendingRx>,
    pub tx_log: Vec<TxRecord>,
}

impl ChipModel {
    fn new() -> Self {
        Self {
            mode: Mode::Standby,
            registers: HashMap::new(),
            buffer: [0; 256],
            tx_base_addr: 0,
            payload_length: 0,
            frequency_raw: 0,
            irq_status: 0,
            irq_mask: 0,
            dio1_mask: 0,
            rx_len: 0,
            cad_activity: false,
            pending_rx: None,
            tx_log: vec![],
        }
    }

    fn raise_irq(&mut self, irq: u16) {
        self.irq_status |= irq & self.irq_mask;
    }

    fn reg_read(&self, addr: u16) -> u8 {
        *self.registers.get(&addr).unwrap_or(&0)
    }

    /// Execute one command; `params` excludes the opcode byte. Returns the
    /// response bytes for opcodes the driver reads from (status byte first).
    fn execute(&mut self, opcode: u8, params: &[u8]) -> Vec<u8> {
        const STATUS_OK: u8 = 0x00;
        match opcode {
            WRITE_REGISTER => {
                let addr = u16::from_be_bytes([params[0], params[1]]);
                for (i, b) in params[2..].iter().enumerate() {
                    self.registers.insert(addr + i as u16, *b);
                }
                vec![]
            }
            READ_REGISTER => {
                // params: addr(2) + nop; length comes from the driver's read buffer
                let addr = u16::from_be_bytes([params[0], params[1]]);
                (0..=u8::MAX).map(|i| self.reg_read(addr + i as u16)).collect()
            }
            WRITE_BUFFER => {
                let offset = params[0] as usize;
                self.buffer[offset..offset + params[1..].len()].copy_from_slice(&params[1..]);
                vec![]
            }
            READ_BUFFER => {
                // No status prefix: the driver reads this via plain read(),
                // clocking the status byte out during its trailing nop write
                let offset = params[0] as usize;
                self.buffer[offset..].to_vec()
            }
            SET_SLEEP => {
                self.mode = Mode::Sleep;
                vec![]
            }
            SET_STANDBY => {
                self.mode = Mode::Standby;
                vec![]
            }
            SET_RF_FREQUENCY => {
                self.frequency_raw = u32::from_be_bytes([params[0], params[1], params[2], params[3]]);
                vec![]
            }
            SET_BUFFER_BASE_ADDRESS => {
                self.tx_base_addr = params[0];
                vec![]
            }
            SET_PACKET_PARAMS => {
                // LoRa: preamble(2), header type, payload length, crc, invert IQ
                self.payload_length = params[3];
                vec![]
            }
            CFG_DIO_IRQ => {
                self.irq_mask = u16::from_be_bytes([params[0], params[1]]);
                self.dio1_mask = u16::from_be_bytes([params[2], params[3]]);
                vec![]
            }
            CLR_IRQ_STATUS => {
                self.irq_status &= !u16::from_be_bytes([params[0], params[1]]);
                vec![]
            }
            GET_IRQ_STATUS => {
                let [hi, lo] = self.irq_status.to_be_bytes();
                vec![STATUS_OK, hi, lo]
            }
            SET_TX => {
                let base = self.tx_base_addr as usize;
                self.tx_log.push(TxRecord {
                    frequency_raw: self.frequency_raw,
                    payload: self.buffer[base..base + self.payload_length as usize].to_vec(),
                });
                // Transmission completes instantly; chip falls back to standby
                self.mode = Mode::Standby;
                self.raise_irq(IRQ_TX_DONE);
                vec![]
            }
            SET_RX => {
                self.mode = Mode::Rx;
                if let Some(rx) = self.pending_rx.take() {
                    self.buffer[..rx.payload.len()].copy_from_slice(&rx.payload);
                    self.rx_len = rx.payload.len() as u8;
                    self.registers.insert(REG_RSSI_SHADOW, (-2 * rx.rssi_dbm) as u8);
                    self.registers.insert(REG_SNR_SHADOW, (4 * rx.snr_db) as u8);
                    self.raise_irq(IRQ_RX_DONE);
                } else if params[..3] != [0xFF, 0xFF, 0xFF] {
                    // Anything but continuous mode times out when no packet
                    // is pending. The wait is collapsed to zero — the model
                    // has no clock, and nothing can arrive once SetRx has
                    // executed — so this exercises the driver's timeout path
                    // (single-mode symbol timeout and timed RX alike), not
                    // timeout durations.
                    self.mode = Mode::Standby;
                    self.raise_irq(IRQ_RX_TX_TIMEOUT);
                }
                vec![]
            }
            GET_RX_BUFFER_STATUS => vec![STATUS_OK, self.rx_len, 0x00],
            GET_PACKET_STATUS => vec![
                STATUS_OK,
                self.reg_read(REG_RSSI_SHADOW),
                self.reg_read(REG_SNR_SHADOW),
                self.reg_read(REG_RSSI_SHADOW),
            ],
            SET_CAD => {
                if self.cad_activity {
                    self.raise_irq(IRQ_CAD_DONE | IRQ_CAD_DETECTED);
                } else {
                    self.raise_irq(IRQ_CAD_DONE);
                }
                vec![]
            }
            // Configuration commands the model accepts without side effects
            _ => vec![STATUS_OK; 16],
        }
    }
}

// Model-internal scratch space for rx metadata, outside the chip's real
// register map (real addresses are all < 0x0A00)
const REG_RSSI_SHADOW: u16 = 0xFF00;
const REG_SNR_SHADOW: u16 = 0xFF01;

/// Test handle to the chip shared by the fake SPI bus and IRQ lines.
#[derive(Clone)]
pub struct Chip(Arc<Mutex<ChipModel>>);

impl Chip {
    pub fn new() -> Self {
        Chip(Arc::new(Mutex::new(ChipModel::new())))
    }

    pub fn inject_rx(&self, payload: &[u8], rssi_dbm: i16, snr_db: i16) {
        self.0.lock().unwrap().pending_rx = Some(PendingRx {
            payload: payload.to_vec(),
            rssi_dbm,
            snr_db,
        });
    }

    pub fn with_model<R>(&self, f: impl FnOnce(&mut ChipModel) -> R) -> R {
        f(&mut self.0.lock().unwrap())
    }
}

/// SPI bus wired to the chip model: writes execute commands, reads drain the
/// command's response bytes (status byte first, matching read_with_status).
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
        let mut response = if cmd.is_empty() {
            vec![]
        } else {
            self.0 .0.lock().unwrap().execute(cmd[0], &cmd[1..])
        };
        let mut cursor = response.drain(..);
        for op in operations.iter_mut() {
            if let Operation::Read(buf) = op {
                for b in buf.iter_mut() {
                    *b = cursor.next().unwrap_or(0);
                }
            }
        }
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

/// IRQ/busy/reset lines wired to the same chip model.
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
        for _ in 0..1000 {
            if self.0.with_model(|m| m.irq_status & m.dio1_mask != 0) {
                return Ok(());
            }
            tokio::task::yield_now().await;
        }
        // The chip model has no pending IRQ and nothing left to raise one
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

pub fn get_emulated_sx1261(chip: &Chip) -> Sx126x<FakeSpi, FakeIv, Sx1261> {
    Sx126x::new(
        FakeSpi(chip.clone()),
        FakeIv(chip.clone()),
        Config {
            chip: Sx1261,
            tcxo_ctrl: None,
            use_dcdc: false,
            rx_boost: true,
        },
    )
}
