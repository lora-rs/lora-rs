//! LR11xx-specific SPI interface implementation
//!
//! The LR11xx family (LR1110, LR1120, LR1121) uses a different SPI protocol
//! than the SX126x/SX127x radios:
//!
//! - Commands are sent in separate SPI transactions from reads
//! - Before every command, wait for BUSY to go low (LR1121 UM §3)
//! - For reads: wait for BUSY again after sending the command, then read
//!   the response in a new transaction
//! - Response always starts with a Stat1 status byte
//!
//! Reference: SWDR001 LR11xx Driver HAL specification

use embedded_hal_async::spi::{Operation, SpiDevice};

use crate::mod_params::RadioError::{self, SPI};
use crate::mod_traits::InterfaceVariant;

pub(crate) struct Lr1110SpiInterface<SPI, IV> {
    pub(crate) spi: SPI,
    pub(crate) iv: IV,
}

impl<SPI, IV> Lr1110SpiInterface<SPI, IV>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
{
    pub fn new(spi: SPI, iv: IV) -> Self {
        Self { spi, iv }
    }

    // Write a buffer to the radio. Pre-waits for BUSY low per LR1121 UM §3.
    // `is_sleep_command` skips the pre-wait for wake pulses where BUSY is
    // held HIGH during sleep.
    pub async fn write(&mut self, write_buffer: &[u8], is_sleep_command: bool) -> Result<(), RadioError> {
        if !is_sleep_command {
            self.iv.wait_on_busy().await?;
        }
        self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        trace!("write: {=[u8]:02x}", write_buffer);

        Ok(())
    }

    // Write command with payload appended. Pre-waits as `write()`.
    pub async fn write_with_payload(
        &mut self,
        write_buffer: &[u8],
        payload: &[u8],
        is_sleep_command: bool,
    ) -> Result<(), RadioError> {
        if !is_sleep_command {
            self.iv.wait_on_busy().await?;
        }
        let mut ops = [Operation::Write(write_buffer), Operation::Write(payload)];
        self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;
        trace!("write_buf: {=[u8]:02x} -> {=[u8]:02x}", write_buffer, payload);

        Ok(())
    }

    // Request a read, filling the provided buffer.
    // For LR11xx this is a two-transaction operation per the HAL spec:
    // 1. Pre-wait, then write command (NSS low -> write -> NSS high)
    // 2. Wait for BUSY to go low (chip preparing response)
    // 3. Read response (NSS low -> read with NOPs -> NSS high)
    // The first byte of the response is Stat1, followed by the actual data.
    pub async fn read(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<(), RadioError> {
        // Step 1: Pre-wait, then write command in separate transaction
        self.iv.wait_on_busy().await?;
        if !write_buffer.is_empty() {
            self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        }

        // Step 2: Wait for BUSY to go low (chip preparing response on MISO)
        self.iv.wait_on_busy().await?;

        // Step 3: Read response; first byte is Stat1 (discarded), rest is data
        let mut stat1 = [0u8; 1];
        let mut ops = [Operation::Read(&mut stat1), Operation::Read(read_buffer)];
        self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;

        trace!(
            "read: addr={=[u8]:02x}, len={}, data={=[u8]:02x}",
            write_buffer,
            read_buffer.len(),
            read_buffer
        );

        Ok(())
    }

    // Request a read with status, returning the Stat1 byte. Same two-transaction
    // protocol as `read()`.
    pub async fn read_with_status(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<u8, RadioError> {
        // Step 1: Pre-wait, then write command in separate transaction
        self.iv.wait_on_busy().await?;
        if !write_buffer.is_empty() {
            self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        }

        // Step 2: Wait for BUSY to go low (chip preparing response on MISO)
        self.iv.wait_on_busy().await?;

        // Step 3: Read response; first byte is Stat1, rest is data
        let mut stat1 = [0u8; 1];
        let mut ops = [Operation::Read(&mut stat1), Operation::Read(read_buffer)];
        self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;

        trace!(
            "read: addr={=[u8]:02x}, len={}, status={:02x}, buf={=[u8]:02x}",
            write_buffer,
            read_buffer.len(),
            stat1[0],
            read_buffer
        );

        Ok(stat1[0])
    }

    // Direct read from SPI bus (no command write phase). Used by
    // lr11xx_system_get_status to read stat1+stat2+irq_status without skipping
    // any bytes.
    pub async fn direct_read(&mut self, read_buffer: &mut [u8]) -> Result<(), RadioError> {
        self.iv.wait_on_busy().await?;
        self.spi.read(read_buffer).await.map_err(|_| SPI)?;

        trace!("direct_read: len={}, data={=[u8]:02x}", read_buffer.len(), read_buffer);

        Ok(())
    }

    // Wake the LR11xx from sleep mode by pulsing NSS. BUSY is held HIGH in
    // sleep, so this can't pre-wait — the next SPI op's pre-wait will catch
    // the chip once it finishes waking. Reference: SWDR001 lr11xx_hal_wakeup().
    pub async fn wakeup(&mut self) -> Result<(), RadioError> {
        let mut dummy = [0u8; 1];
        self.spi.read(&mut dummy).await.map_err(|_| SPI)?;

        trace!("wakeup: NSS pulsed to wake chip");

        Ok(())
    }
}
