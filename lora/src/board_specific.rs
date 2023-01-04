use embedded_hal_async::delay::DelayUs;
use embedded_hal_async::spi::*;

use crate::InterfaceVariant;

use super::mod_params::RadioError::*;
use super::mod_params::*;
use super::LoRa;

// Defines the time required for the TCXO to wakeup [ms].
const BRD_TCXO_WAKEUP_TIME: u32 = 10;

// Provides board-specific functionality for Semtech SX126x-based boards.

impl<SPI> LoRa<SPI>
where
    SPI: SpiBus<u8> + 'static,
{
    // De-initialize the radio I/Os pins interface.  Useful when going into MCU low power modes.
    pub(super) async fn brd_io_deinit(
        &mut self,
        _iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        Ok(()) // no operation currently
    }

    // Initialize the TCXO power pin
    pub(super) async fn brd_io_tcxo_init(
        &mut self,
        iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        let timeout = self.brd_get_board_tcxo_wakeup_time() << 6;
        self.sub_set_dio3_as_tcxo_ctrl(iv, TcxoCtrlVoltage::Ctrl1V7, timeout)
            .await?;
        Ok(())
    }

    // Initialize RF switch control pins
    pub(super) async fn brd_io_rf_switch_init(
        &mut self,
        iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        self.sub_set_dio2_as_rf_switch_ctrl(iv, true).await?;
        Ok(())
    }

    // Initialize the radio debug pins
    pub(super) async fn brd_io_dbg_init(
        &mut self,
        _iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        Ok(()) // no operation currently
    }

    // Hardware reset of the radio
    pub(super) async fn brd_reset(
        &mut self,
        iv: &mut impl InterfaceVariant,
        _delay: &mut impl DelayUs,
    ) -> Result<(), RadioError> {
        iv.reset().await
        /*
        delay.delay_ms(10).await.map_err(|_| DelayError)?;
        self.reset.set_low().map_err(|_| Reset)?;
        delay.delay_ms(20).await.map_err(|_| DelayError)?;
        self.reset.set_high().map_err(|_| Reset)?;
        delay.delay_ms(10).await.map_err(|_| DelayError)?;
        Ok(())
        */
    }

    // Wait while the busy pin is high
    pub(super) async fn brd_wait_on_busy(
        &mut self,
        _iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        /*
        self.busy.wait_for_low().await.map_err(|_| Busy)?;
        */
        Ok(())
    }

    // Wake up the radio
    pub(super) async fn brd_wakeup(
        &mut self,
        iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        iv.set_nss_low().await?;
        self.spi
            .write(&[OpCode::GetStatus.value()])
            .await
            .map_err(|_| SPI)?;
        self.spi.write(&[0x00]).await.map_err(|_| SPI)?;
        iv.set_nss_high().await?;

        iv.wait_on_busy().await?;
        self.brd_set_operating_mode(RadioMode::StandbyRC);
        Ok(())
    }

    // Send a command that writes data to the radio
    pub(super) async fn brd_write_command(
        &mut self,
        iv: &mut impl InterfaceVariant,
        op_code: OpCode,
        buffer: &[u8],
    ) -> Result<(), RadioError> {
        self.sub_check_device_ready(iv).await?;

        iv.set_nss_low().await?;
        self.spi.write(&[op_code.value()]).await.map_err(|_| SPI)?;
        self.spi.write(buffer).await.map_err(|_| SPI)?;
        iv.set_nss_high().await?;

        if op_code != OpCode::SetSleep {
            iv.wait_on_busy().await?;
        }
        Ok(())
    }

    // Send a command that reads data from the radio, filling the provided buffer and returning a status
    pub(super) async fn brd_read_command(
        &mut self,
        iv: &mut impl InterfaceVariant,
        op_code: OpCode,
        buffer: &mut [u8],
    ) -> Result<u8, RadioError> {
        let mut status = [0u8];
        let mut input = [0u8];

        self.sub_check_device_ready(iv).await?;

        iv.set_nss_low().await?;
        self.spi.write(&[op_code.value()]).await.map_err(|_| SPI)?;
        self.spi
            .transfer(&mut status, &[0x00])
            .await
            .map_err(|_| SPI)?;
        for i in 0..buffer.len() {
            self.spi
                .transfer(&mut input, &[0x00])
                .await
                .map_err(|_| SPI)?;
            buffer[i] = input[0];
        }
        iv.set_nss_high().await?;

        iv.wait_on_busy().await?;

        Ok(status[0])
    }

    // Write one or more bytes of data to the radio memory
    pub(super) async fn brd_write_registers(
        &mut self,
        iv: &mut impl InterfaceVariant,
        start_register: Register,
        buffer: &[u8],
    ) -> Result<(), RadioError> {
        self.sub_check_device_ready(iv).await?;

        iv.set_nss_low().await?;
        self.spi
            .write(&[OpCode::WriteRegister.value()])
            .await
            .map_err(|_| SPI)?;
        self.spi
            .write(&[
                ((start_register.addr() & 0xFF00) >> 8) as u8,
                (start_register.addr() & 0x00FF) as u8,
            ])
            .await
            .map_err(|_| SPI)?;
        self.spi.write(buffer).await.map_err(|_| SPI)?;
        iv.set_nss_high().await?;

        iv.wait_on_busy().await
    }

    // Read one or more bytes of data from the radio memory
    pub(super) async fn brd_read_registers(
        &mut self,
        iv: &mut impl InterfaceVariant,
        start_register: Register,
        buffer: &mut [u8],
    ) -> Result<(), RadioError> {
        let mut input = [0u8];

        self.sub_check_device_ready(iv).await?;

        iv.set_nss_low().await?;
        self.spi
            .write(&[OpCode::ReadRegister.value()])
            .await
            .map_err(|_| SPI)?;
        self.spi
            .write(&[
                ((start_register.addr() & 0xFF00) >> 8) as u8,
                (start_register.addr() & 0x00FF) as u8,
                0x00u8,
            ])
            .await
            .map_err(|_| SPI)?;
        for i in 0..buffer.len() {
            self.spi
                .transfer(&mut input, &[0x00])
                .await
                .map_err(|_| SPI)?;
            buffer[i] = input[0];
        }
        iv.set_nss_high().await?;

        iv.wait_on_busy().await
    }

    // Write data to the buffer holding the payload in the radio
    pub(super) async fn brd_write_buffer(
        &mut self,
        iv: &mut impl InterfaceVariant,
        offset: u8,
        buffer: &[u8],
    ) -> Result<(), RadioError> {
        self.sub_check_device_ready(iv).await?;

        iv.set_nss_low().await?;
        self.spi
            .write(&[OpCode::WriteBuffer.value()])
            .await
            .map_err(|_| SPI)?;
        self.spi.write(&[offset]).await.map_err(|_| SPI)?;
        self.spi.write(buffer).await.map_err(|_| SPI)?;
        iv.set_nss_high().await?;

        iv.wait_on_busy().await
    }

    // Read data from the buffer holding the payload in the radio
    pub(super) async fn brd_read_buffer(
        &mut self,
        iv: &mut impl InterfaceVariant,
        offset: u8,
        buffer: &mut [u8],
    ) -> Result<(), RadioError> {
        let mut input = [0u8];

        self.sub_check_device_ready(iv).await?;

        iv.set_nss_low().await?;
        self.spi
            .write(&[OpCode::ReadBuffer.value()])
            .await
            .map_err(|_| SPI)?;
        self.spi.write(&[offset]).await.map_err(|_| SPI)?;
        self.spi.write(&[0x00]).await.map_err(|_| SPI)?;
        for i in 0..buffer.len() {
            self.spi
                .transfer(&mut input, &[0x00])
                .await
                .map_err(|_| SPI)?;
            buffer[i] = input[0];
        }
        iv.set_nss_high().await?;

        iv.wait_on_busy().await
    }

    // Set the radio output power
    pub(super) async fn brd_set_rf_tx_power(
        &mut self,
        iv: &mut impl InterfaceVariant,
        power: i8,
    ) -> Result<(), RadioError> {
        self.sub_set_tx_params(iv, power, RampTime::Ramp40Us)
            .await?;
        Ok(())
    }

    // Get the radio type
    pub(super) fn brd_get_radio_type(&mut self) -> RadioType {
        RadioType::SX1262
    }

    // Quiesce the antenna(s).
    pub(super) fn brd_ant_sleep(
        &mut self,
        _iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        // self.antenna_tx.set_low().map_err(|_| AntTx)?;
        // self.antenna_rx.set_low().map_err(|_| AntRx)?;
        Ok(())
    }

    // Prepare the antenna(s) for a receive operation
    pub(super) fn brd_ant_set_rx(
        &mut self,
        _iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        // self.antenna_tx.set_low().map_err(|_| AntTx)?;
        // self.antenna_rx.set_high().map_err(|_| AntRx)?;
        Ok(())
    }

    // Prepare the antenna(s) for a send operation
    pub(super) fn brd_ant_set_tx(
        &mut self,
        _iv: &mut impl InterfaceVariant,
    ) -> Result<(), RadioError> {
        // self.antenna_rx.set_low().map_err(|_| AntRx)?;
        // self.antenna_tx.set_high().map_err(|_| AntTx)?;
        Ok(())
    }

    // Check if the given RF frequency is supported by the hardware
    pub(super) async fn brd_check_rf_frequency(
        &mut self,
        _iv: &mut impl InterfaceVariant,
        _frequency: u32,
    ) -> Result<bool, RadioError> {
        Ok(true)
    }

    // Get the duration required for the TCXO to wakeup [ms].
    pub(super) fn brd_get_board_tcxo_wakeup_time(&mut self) -> u32 {
        BRD_TCXO_WAKEUP_TIME
    }

    /* Get current state of the DIO1 pin - not currently needed if waiting on DIO1 instead of using an IRQ process
    pub(super) async fn brd_get_dio1_pin_state(
        &mut self,
    ) -> Result<u32, RadioError> {
        Ok(0)
    }
    */

    // Get the current radio operatiing mode
    pub(super) fn brd_get_operating_mode(&mut self) -> RadioMode {
        self.operating_mode
    }

    // Set/Update the current radio operating mode  This function is only required to reflect the current radio operating mode when processing interrupts.
    pub(super) fn brd_set_operating_mode(&mut self, mode: RadioMode) {
        self.operating_mode = mode;
    }
}
