use crate::mod_params::RadioError;
use crate::mod_traits::InterfaceVariant;
use crate::sx127x::{Config, Sx1276, Sx127x};
use embedded_hal::spi::{ErrorKind, Operation};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::SpiDevice;
use std::collections::{BTreeMap, VecDeque};

/// Register-file SPI fixture for the sx127x.
///
/// The sx127x is register-based and the two drivers legitimately factor
/// their register traffic differently: the reference burst-writes runs of
/// registers (`Frf` in one transaction, auto-increment) and reads registers
/// back for read-modify-write where ours keeps shadow copies (and vice
/// versa). Comparing raw transactions would fail on equivalent behavior, so
/// this fixture *executes* the traffic against a small register file instead:
/// writes update it (auto-incrementing across a burst), reads answer from it.
///
/// Equality compares chip-visible outcomes: the final register file, the
/// byte stream written to the FIFO (address 0x00, no auto-increment), and
/// the write-1-to-clear bits pushed at RegIrqFlags (kept out of the register
/// file since writing 0xFF there clears flags rather than storing 0xFF).
#[derive(Clone, Debug)]
pub struct TestFixture {
    regs: BTreeMap<u8, u8>,
    pub fifo_written: Vec<u8>,
    pub irq_flags_written: Vec<u8>,
    fifo_read: VecDeque<u8>,
}

// Deliberately hardcoded (datasheet addresses), NOT derived from the
// driver's Register enum: the special-case handling below keys off these,
// and FIFO/IRQ traffic is excluded from the register-file equality. If a
// driver enum value went wrong and these followed it, the special-casing
// would absorb the misdirected write instead of the comparison catching it.
const REG_FIFO: u8 = 0x00;
const REG_OP_MODE: u8 = 0x01;
const REG_IRQ_FLAGS: u8 = 0x12;

/// SX1276 power-on defaults (datasheet table 41) for every register either
/// driver touches in the compared flows; both sides start from the same file.
const RESET_VALUES: &[(u8, u8)] = &[
    (0x01, 0x80), // RegOpMode: LoRa, sleep (both drivers enter LoRa mode from reset)
    (0x06, 0x6C), // RegFrfMsb..Lsb: 434 MHz
    (0x07, 0x80),
    (0x08, 0x00),
    (0x09, 0x4F), // RegPaConfig
    (0x0A, 0x09), // RegPaRamp
    (0x0B, 0x2B), // RegOcp
    (0x0C, 0x20), // RegLna
    (0x0D, 0x00), // RegFifoAddrPtr
    (0x0E, 0x80), // RegFifoTxBaseAddr
    (0x0F, 0x00), // RegFifoRxBaseAddr
    (0x11, 0x00), // RegIrqFlagsMask
    (0x1D, 0x72), // RegModemConfig1
    (0x1E, 0x70), // RegModemConfig2
    (0x1F, 0x64), // RegSymbTimeoutLsb
    (0x20, 0x00), // RegPreambleMsb
    (0x21, 0x08), // RegPreambleLsb
    (0x22, 0x01), // RegPayloadLength
    (0x26, 0x00), // RegModemConfig3
    (0x31, 0xC3), // RegDetectionOptimize
    (0x33, 0x27), // RegInvertiq
    (0x37, 0x0A), // RegDetectionThreshold
    (0x39, 0x12), // RegSyncWord
    (0x3B, 0x1D), // RegInvertiq2
    (0x40, 0x00), // RegDioMapping1
    (0x41, 0x00), // RegDioMapping2
    (0x42, 0x12), // RegVersion
    (0x4D, 0x84), // RegPaDac (sx1276)
];

impl PartialEq for TestFixture {
    fn eq(&self, other: &Self) -> bool {
        // irq_flags_written is deliberately NOT compared: the reference
        // driver clears IRQ flags at the chip only inside its DIO interrupt
        // handlers (its public clear_irq_status is shadow-state only), so
        // the W1C traffic has no C counterpart in these flows. Tests assert
        // ours' clears explicitly where they matter.
        self.regs == other.regs && self.fifo_written == other.fifo_written
    }
}
impl Eq for TestFixture {}

impl Default for TestFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl TestFixture {
    pub fn new() -> Self {
        Self {
            regs: RESET_VALUES.iter().copied().collect(),
            fifo_written: vec![],
            irq_flags_written: vec![],
            fifo_read: VecDeque::new(),
        }
    }

    /// Override a register value (e.g. status registers before a read test)
    pub fn set_reg(&mut self, address: u8, value: u8) {
        self.regs.insert(address, value);
    }

    pub fn reg(&self, address: u8) -> u8 {
        *self.regs.get(&address).unwrap_or(&0)
    }

    /// Queue bytes the FIFO will yield on reads
    pub fn prime_fifo_read(&mut self, data: &[u8]) {
        self.fifo_read.extend(data);
    }

    fn record(&mut self, operations: &mut [Operation<'_, u8>]) {
        let mut cmd = Vec::new();
        for op in operations.iter() {
            if let Operation::Write(buf) = op {
                cmd.extend_from_slice(buf)
            }
        }
        let has_read = operations.iter().any(|op| matches!(op, Operation::Read(_)));
        assert!(!cmd.is_empty(), "sx127x transaction with no address byte");
        let wnr = cmd[0] & 0x80 != 0;
        let address = cmd[0] & 0x7F;

        if has_read {
            assert!(!wnr, "read transaction with wnr bit set: {cmd:02x?}");
            assert_eq!(cmd.len(), 1, "bytes written during a read: {cmd:02x?}");
            let mut offset = 0u8;
            for op in operations.iter_mut() {
                if let Operation::Read(buf) = op {
                    for b in buf.iter_mut() {
                        *b = if address == REG_FIFO {
                            self.fifo_read.pop_front().unwrap_or(0)
                        } else {
                            self.reg(address + offset)
                        };
                        offset += 1;
                    }
                }
            }
        } else {
            assert!(wnr, "write transaction without wnr bit: {cmd:02x?}");
            for (i, b) in cmd[1..].iter().enumerate() {
                let mut b = *b;
                if address == REG_FIFO {
                    self.fifo_written.push(b);
                    continue;
                }
                if address == REG_IRQ_FLAGS {
                    self.irq_flags_written.push(b);
                    continue;
                }
                if address + (i as u8) == REG_OP_MODE {
                    // LongRangeMode is only writable while the device is in
                    // sleep AND the same write keeps it there; otherwise the
                    // chip ignores bit 7. SWL2001 writes mode-only values
                    // (bit 7 clear) relying on this; our driver rewrites the
                    // full byte. Both land on the same device state.
                    let current = self.reg(REG_OP_MODE);
                    let in_sleep = current & 0x07 == 0;
                    let write_stays_in_sleep = b & 0x07 == 0;
                    if !(in_sleep && write_stays_in_sleep) {
                        b = (current & 0x80) | (b & 0x7F);
                    }
                }
                self.regs.insert(address + i as u8, b);
            }
        }
    }
}

#[derive(Debug)]
pub enum Error {}
impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> ErrorKind {
        todo!()
    }
}
impl embedded_hal::spi::ErrorType for TestFixture {
    type Error = Error;
}

impl embedded_hal::spi::SpiDevice for TestFixture {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.record(operations);
        Ok(())
    }
}

impl SpiDevice<u8> for TestFixture {
    async fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut ops = [Operation::Write(buf)];
        self.record(&mut ops);
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

/// SX1276, no TCXO, RFO output, no RX boost
pub fn get_sx1276() -> Sx127x<TestFixture, DummyVariant, Sx1276> {
    Sx127x::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1276,
            tcxo_used: false,
            tx_boost: false,
            rx_boost: false,
        },
    )
}

/// SX1276 with PA_BOOST output
pub fn get_sx1276_boost() -> Sx127x<TestFixture, DummyVariant, Sx1276> {
    Sx127x::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1276,
            tcxo_used: false,
            tx_boost: true,
            rx_boost: false,
        },
    )
}

pub struct Delayer;
impl DelayNs for Delayer {
    async fn delay_ns(&mut self, _ns: u32) {}
}

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
