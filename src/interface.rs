use defmt::info;
use embedded_hal_async::spi::SpiBus;

use crate::mod_params::RadioError;
use crate::mod_params::RadioError::*;
use crate::mod_traits::InterfaceVariant;

pub(crate) struct SpiInterface<SPI, IV> {
    pub(crate) spi: SPI,
    pub(crate) iv: IV,
}

impl<SPI, IV> SpiInterface<SPI, IV>
where
    SPI: SpiBus<u8> + 'static,
    IV: InterfaceVariant + 'static,
{
    pub fn new(spi: SPI, iv: IV) -> Self {
        Self { spi, iv }
    }

    // Write one or more buffers to the radio.
    pub async fn write(&mut self, write_buffers: &[&[u8]], is_sleep_command: bool) -> Result<(), RadioError> {
        self.iv.set_nss_low().await?;
        for buffer in write_buffers {
            let write_result = self.spi.write(buffer).await.map_err(|_| SPI);
            let flush_result = self.spi.flush().await.map_err(|_| SPI);
            if write_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                write_result?;
            } else if flush_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                flush_result?;
            }
        }
        self.iv.set_nss_high().await?;

        if !is_sleep_command {
            self.iv.wait_on_busy().await?;
        }

        // debug ???
        match write_buffers.len() {
            1 => info!("write: 0x{:x}", write_buffers[0]),
            2 => info!("write: 0x{:x} 0x{:x}", write_buffers[0], write_buffers[1]),
            3 => info!("write: 0x{:x} 0x{:x} 0x{:x}", write_buffers[0], write_buffers[1], write_buffers[2]),
            _ => info!("write: too many buffers"),
        }
        
        Ok(())
    }

    // Request a read, filling the provided buffer.
    pub async fn read(
        &mut self,
        write_buffers: &[&[u8]],
        read_buffer: &mut [u8],
        read_length: Option<u8>,
    ) -> Result<(), RadioError> {
        let mut input = [0u8];

        self.iv.set_nss_low().await?;
        for buffer in write_buffers {
            let write_result = self.spi.write(buffer).await.map_err(|_| SPI);
            let flush_result = self.spi.flush().await.map_err(|_| SPI);
            if write_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                write_result?;
            } else if flush_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                flush_result?;
            }
        }

        let number_to_read = match read_length {
            Some(len) => len as usize,
            None => read_buffer.len(),
        };

        for i in 0..number_to_read {
            let transfer_result = self.spi.transfer(&mut input, &[0x00]).await.map_err(|_| SPI);
            let flush_result = self.spi.flush().await.map_err(|_| SPI);
            if transfer_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                transfer_result?;
            } else if flush_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                flush_result?;
            }
            read_buffer[i] = input[0];
        }
        self.iv.set_nss_high().await?;

        self.iv.wait_on_busy().await?;

        // debug ???
        match write_buffers.len() {
            1 => info!("write: 0x{:x}", write_buffers[0]),
            2 => info!("write: 0x{:x} 0x{:x}", write_buffers[0], write_buffers[1]),
            3 => info!("write: 0x{:x} 0x{:x} 0x{:x}", write_buffers[0], write_buffers[1], write_buffers[2]),
            _ => info!("write: too many buffers"),
        }
        info!("read {}: 0x{:x}", number_to_read, read_buffer);

        Ok(())
    }

    // Request a read with status, filling the provided buffer and returning the status.
    pub async fn read_with_status(
        &mut self,
        write_buffers: &[&[u8]],
        read_buffer: &mut [u8],
    ) -> Result<u8, RadioError> {
        let mut status = [0u8];
        let mut input = [0u8];

        self.iv.set_nss_low().await?;
        for buffer in write_buffers {
            let write_result = self.spi.write(buffer).await.map_err(|_| SPI);
            let flush_result = self.spi.flush().await.map_err(|_| SPI);
            if write_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                write_result?;
            } else if flush_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                flush_result?;
            }
        }

        let transfer_result = self.spi.transfer(&mut status, &[0x00]).await.map_err(|_| SPI);
        let flush_result = self.spi.flush().await.map_err(|_| SPI);
        if transfer_result != Ok(()) {
            let _err = self.iv.set_nss_high().await;
            transfer_result?;
        } else if flush_result != Ok(()) {
            let _err = self.iv.set_nss_high().await;
            flush_result?;
        }

        for i in 0..read_buffer.len() {
            let transfer_result = self.spi.transfer(&mut input, &[0x00]).await.map_err(|_| SPI);
            let flush_result = self.spi.flush().await.map_err(|_| SPI);
            if transfer_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                transfer_result?;
            } else if flush_result != Ok(()) {
                let _err = self.iv.set_nss_high().await;
                flush_result?;
            }
            read_buffer[i] = input[0];
        }
        self.iv.set_nss_high().await?;

        self.iv.wait_on_busy().await?;

        // debug ???
        match write_buffers.len() {
            1 => info!("write: 0x{:x}", write_buffers[0]),
            2 => info!("write: 0x{:x} 0x{:x}", write_buffers[0], write_buffers[1]),
            3 => info!("write: 0x{:x} 0x{:x} 0x{:x}", write_buffers[0], write_buffers[1], write_buffers[2]),
            _ => info!("write: too many buffers"),
        }
        info!("read {} status 0x{:x}: 0x{:x}", read_buffer.len(), status[0], read_buffer);

        Ok(status[0])
    }
}
