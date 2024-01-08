use defmt::trace;
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

    // Write a buffer to the radio.
    pub async fn write(&mut self, write_buffer: &[u8], is_sleep_command: bool) -> Result<(), RadioError> {
        self.spi.write(write_buffer).await.map_err(|_| SPI)?;
        trace!("write: {=[u8]:02x}", write_buffer);

        if !is_sleep_command {
            self.iv.wait_on_busy().await?;
        }

        Ok(())
    }

    // Write
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
    pub async fn read(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<(), RadioError> {
        {
            let mut ops = [Operation::Write(write_buffer), Operation::Read(read_buffer)];

            self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;
        }

        self.iv.wait_on_busy().await?;

        trace!(
            "read: addr={=[u8]:02x}, len={}, data={=[u8]:02x}",
            write_buffer,
            read_buffer.len(),
            read_buffer
        );

        Ok(())
    }

    // Request a read with status, filling the provided buffer and returning the status.
    pub async fn read_with_status(&mut self, write_buffer: &[u8], read_buffer: &mut [u8]) -> Result<u8, RadioError> {
        let mut status = [0u8];
        {
            let mut ops = [
                Operation::Write(write_buffer),
                Operation::Read(&mut status),
                Operation::Read(read_buffer),
            ];

            self.spi.transaction(&mut ops).await.map_err(|_| SPI)?;
        }

        self.iv.wait_on_busy().await?;

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
