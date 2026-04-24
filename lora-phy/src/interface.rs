use embedded_hal_async::spi::{Operation, SpiDevice};

use crate::mod_params::RadioError::{self, SPI};
use crate::mod_traits::InterfaceVariant;

pub(crate) struct SpiInterface<SPI, IV> {
    pub(crate) spi: SPI,
    pub(crate) iv: IV,
}

impl<SPI, IV> SpiInterface<SPI, IV>
where
    SPI: SpiDevice<u8>,
    IV: InterfaceVariant,
{
    pub fn new(spi: SPI, iv: IV) -> Self {
        Self { spi, iv }
    }

    // Write a buffer to the radio. Pre-waits for BUSY low per SX1261/2 §8.3.1.
    // `is_sleep_command` skips the pre-wait for the wake-from-sleep pulse where
    // BUSY is held HIGH until NSS falls (see `sx126x::ensure_ready`).
    pub async fn write(&mut self, write_buffer: &[u8], is_sleep_command: bool) -> Result<(), RadioError> {
        if !is_sleep_command {
            self.iv.wait_on_busy().await?;
        }
        self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        trace!("write: {=[u8]:02x}", write_buffer);

        Ok(())
    }

    // Write a command buffer followed by an inline payload. Pre-waits as `write()`.
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

    // Request a read, filling the provided buffer. Pre-waits for BUSY low.
    pub async fn read(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<(), RadioError> {
        self.iv.wait_on_busy().await?;

        let mut ops = [Operation::Write(write_buffer), Operation::Read(read_buffer)];
        self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;

        trace!(
            "read: addr={=[u8]:02x}, len={}, data={=[u8]:02x}",
            write_buffer,
            read_buffer.len(),
            read_buffer
        );

        Ok(())
    }

    // Request a read with status, filling the buffer and returning the status. Pre-waits for BUSY low.
    pub async fn read_with_status(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<u8, RadioError> {
        self.iv.wait_on_busy().await?;

        let mut status = [0u8];
        let mut ops = [
            Operation::Write(write_buffer),
            Operation::Read(&mut status),
            Operation::Read(read_buffer),
        ];
        self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;

        trace!(
            "read: addr={=[u8]:02x}, len={}, status={:02x}, buf={=[u8]:02x}",
            write_buffer,
            read_buffer.len(),
            status[0],
            read_buffer
        );

        Ok(status[0])
    }
}
