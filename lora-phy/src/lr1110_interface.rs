//! LR11xx-specific SPI interface implementation
//!
//! The LR11xx family (LR1110, LR1120, LR1121) uses a different SPI protocol
//! than the SX126x/SX127x radios:
//!
//! - Commands are sent in separate SPI transactions from reads
//! - After writing a command, wait for BUSY to go low
//! - Then read the response in a new transaction
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

    // Write a buffer to the radio.
    pub async fn write(&mut self, write_buffer: &[u8], is_sleep_command: bool) -> Result<(), RadioError> {
        // Guard: wait for BUSY to be LOW before asserting NSS
        self.iv.wait_on_busy().await?;

        self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        trace!("write: {=[u8]:02x}", write_buffer);

        if !is_sleep_command {
            self.iv.wait_on_busy().await?;
        }

        Ok(())
    }

    // Write command with payload appended
    pub async fn write_with_payload(
        &mut self,
        write_buffer: &[u8],
        payload: &[u8],
        is_sleep_command: bool,
    ) -> Result<(), RadioError> {
        let mut ops = [Operation::Write(write_buffer), Operation::Write(payload)];
        self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;
        trace!("write_buf: {=[u8]:02x} -> {=[u8]:02x}", write_buffer, payload);

        if !is_sleep_command {
            self.iv.wait_on_busy().await?;
        }

        Ok(())
    }

    // Request a read, filling the provided buffer.
    // For LR11xx: This is a two-step operation per the HAL specification:
    // 1. Write command (NSS low -> write -> NSS high)
    // 2. Wait for BUSY to go low
    // 3. Read response (NSS low -> read with NOPs -> NSS high)
    // The first byte of the response is Stat1, followed by the actual data.
    pub async fn read(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<(), RadioError> {
        // Step 1: Write command in separate transaction
        if !write_buffer.is_empty() {
            // Guard: wait for BUSY to be LOW before asserting NSS. If the
            // previous transaction's BUSY de-assertion glitches or the GPIOTE
            // edge-detection in wait_for_low returns early, writing a new
            // command while BUSY is still asserted will corrupt the response.
            self.iv.wait_on_busy().await?;

            self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        }

        // Step 2: Wait for BUSY to go low (post-command processing)
        self.iv.wait_on_busy().await?;

        // Step 3: Read response in separate transaction
        // First byte is Stat1, followed by actual data.
        let mut stat1 = [0u8; 1];
        let mut ops = [Operation::Read(&mut stat1), Operation::Read(read_buffer)];
        self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;

        trace!(
            "read: addr={=[u8]:02x}, len={}, stat1={:02x}, data={=[u8]:02x}",
            write_buffer,
            read_buffer.len(),
            stat1[0],
            read_buffer
        );

        // Stat1 bits[3:1] encode the command status:
        //   0 = CMD_FAIL  (command could not be executed)
        //   1 = CMD_PERR  (wrong opcode or arguments)
        //   2 = CMD_OK    (success)
        //   3 = CMD_DAT   (success, data available)
        // Anything below CMD_OK means the response data is invalid.
        if (stat1[0] >> 1) & 0x07 < 2 {
            return Err(RadioError::OpError(stat1[0]));
        }

        Ok(())
    }

    // Request a read with status, filling the provided buffer and returning the status.
    // For LR11xx: Same two-step protocol as read(), but returns the Stat1 byte.
    pub async fn read_with_status(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<u8, RadioError> {
        // Step 1: Write command in separate transaction
        if !write_buffer.is_empty() {
            self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        }

        // Step 2: Wait for BUSY to go low
        self.iv.wait_on_busy().await?;

        // Step 3: Read response in separate transaction
        // First byte is Stat1, followed by actual data
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

    // Direct read from SPI bus (no command write phase).
    // For LR11xx: This is used by lr11xx_system_get_status to read stat1+stat2+irq_status.
    // Unlike read(), this does NOT skip any bytes - all bytes read are returned.
    pub async fn direct_read(&mut self, read_buffer: &mut [u8]) -> Result<(), RadioError> {
        // Wait for BUSY to go low
        self.iv.wait_on_busy().await?;

        // Read directly - no stat1 skipping
        self.spi.read(read_buffer).await.map_err(|_| SPI)?;

        trace!("direct_read: len={}, data={=[u8]:02x}", read_buffer.len(), read_buffer);

        Ok(())
    }

    // Wakeup the LR11xx from sleep mode by toggling NSS.
    // For LR11xx: This is the HAL-level wakeup that simply asserts NSS low,
    // waits for BUSY to go low, then de-asserts NSS high.
    // Reference: SWDR001 lr11xx_hal_wakeup()
    pub async fn wakeup(&mut self) -> Result<(), RadioError> {
        // Perform a dummy read with an empty buffer
        // This will assert NSS, wait for BUSY, and de-assert NSS
        let mut dummy = [0u8; 1];
        self.spi.read(&mut dummy).await.map_err(|_| SPI)?;

        // Wait for BUSY to go low
        self.iv.wait_on_busy().await?;

        trace!("wakeup: chip awakened from sleep");

        Ok(())
    }
}
