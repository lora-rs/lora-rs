/// InterfaceVariant implementation for LR1110 on STM32WBA
///
/// This module provides the hardware abstraction layer between the STM32WBA MCU
/// and the LR1110 LoRa radio chip over SPI.
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use lora_phy::DelayNs;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_params::RadioError::*;
use lora_phy::mod_traits::InterfaceVariant;

/// InterfaceVariant for LR1110 on STM32WBA
///
/// Pin connections (example - adjust based on your board):
/// - RESET: GPIO output for resetting the LR1110
/// - BUSY: GPIO input for LR1110 BUSY signal (active high when processing)
/// - DIO1: GPIO input with IRQ for receiving radio events
/// - RF_SWITCH_RX: Optional GPIO for RX antenna switch control
/// - RF_SWITCH_TX: Optional GPIO for TX antenna switch control
///
/// Note: The LR1110 uses DIO0 as the BUSY signal. The BUSY line goes high
/// when the chip is processing a command and is not ready for new commands.
pub struct Stm32wbaLr1110InterfaceVariant<RESET, BUSY, DIO1, CTRL>
where
    RESET: OutputPin,
    BUSY: InputPin + Wait,
    DIO1: Wait,
    CTRL: OutputPin,
{
    reset: RESET,
    busy: BUSY,
    dio1: DIO1,
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<RESET, BUSY, DIO1, CTRL> Stm32wbaLr1110InterfaceVariant<RESET, BUSY, DIO1, CTRL>
where
    RESET: OutputPin,
    BUSY: InputPin + Wait,
    DIO1: Wait,
    CTRL: OutputPin,
{
    /// Create a new InterfaceVariant for LR1110 on STM32WBA
    ///
    /// # Arguments
    /// * `reset` - GPIO pin connected to LR1110 RESET
    /// * `busy` - GPIO pin connected to LR1110 BUSY (DIO0) - active high when processing
    /// * `dio1` - GPIO pin with EXTI connected to LR1110 DIO1 (IRQ)
    /// * `rf_switch_rx` - Optional GPIO for RX antenna switch
    /// * `rf_switch_tx` - Optional GPIO for TX antenna switch
    pub fn new(
        reset: RESET,
        busy: BUSY,
        dio1: DIO1,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        Ok(Self {
            reset,
            busy,
            dio1,
            rf_switch_rx,
            rf_switch_tx,
        })
    }
}

impl<RESET, BUSY, DIO1, CTRL> InterfaceVariant for Stm32wbaLr1110InterfaceVariant<RESET, BUSY, DIO1, CTRL>
where
    RESET: OutputPin,
    BUSY: InputPin + Wait,
    DIO1: Wait,
    CTRL: OutputPin,
{
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        // LR1110 reset sequence:
        // 1. Set RESET low for at least 50us
        // 2. Wait for BUSY to go low (chip ready)
        // 3. Set RESET high
        // 4. Wait for BUSY to go low again (chip ready after reset)
        delay.delay_ms(1).await;
        self.reset.set_low().map_err(|_| Reset)?;
        delay.delay_ms(10).await;
        self.reset.set_high().map_err(|_| Reset)?;
        // Wait for chip to be ready after reset
        self.busy.wait_for_low().await.map_err(|_| Busy)?;
        Ok(())
    }

    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        // LR1110 BUSY pin (DIO0) is high when processing a command
        // Wait for it to go low before sending next command
        self.busy.wait_for_low().await.map_err(|_| Busy)?;
        Ok(())
    }

    async fn await_irq(&mut self) -> Result<(), RadioError> {
        self.dio1.wait_for_high().await.map_err(|_| DIO1)?;
        Ok(())
    }

    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError> {
        // Disable TX switch
        if let Some(pin) = &mut self.rf_switch_tx {
            pin.set_low().map_err(|_| RfSwitchTx)?;
        }
        // Enable RX switch
        if let Some(pin) = &mut self.rf_switch_rx {
            pin.set_high().map_err(|_| RfSwitchRx)?;
        }
        Ok(())
    }

    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError> {
        // Disable RX switch
        if let Some(pin) = &mut self.rf_switch_rx {
            pin.set_low().map_err(|_| RfSwitchRx)?;
        }
        // Enable TX switch
        if let Some(pin) = &mut self.rf_switch_tx {
            pin.set_high().map_err(|_| RfSwitchTx)?;
        }
        Ok(())
    }

    async fn disable_rf_switch(&mut self) -> Result<(), RadioError> {
        // Disable both switches
        if let Some(pin) = &mut self.rf_switch_rx {
            pin.set_low().map_err(|_| RfSwitchRx)?;
        }
        if let Some(pin) = &mut self.rf_switch_tx {
            pin.set_low().map_err(|_| RfSwitchTx)?;
        }
        Ok(())
    }
}
