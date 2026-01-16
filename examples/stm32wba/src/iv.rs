/// InterfaceVariant implementation for LR1110 on STM32WBA
///
/// This module provides the hardware abstraction layer between the STM32WBA MCU
/// and the LR1110 LoRa radio chip over SPI.

use embedded_hal::digital::OutputPin;
use embedded_hal_async::digital::Wait;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_params::RadioError::*;
use lora_phy::mod_traits::InterfaceVariant;
use lora_phy::DelayNs;

/// InterfaceVariant for LR1110 on STM32WBA
///
/// Pin connections (example - adjust based on your board):
/// - RESET: GPIO output for resetting the LR1110
/// - DIO1: GPIO input with IRQ for receiving radio events
/// - RF_SWITCH_RX: Optional GPIO for RX antenna switch control
/// - RF_SWITCH_TX: Optional GPIO for TX antenna switch control
pub struct Stm32wbaLr1110InterfaceVariant<RESET, DIO1, CTRL>
where
    RESET: OutputPin,
    DIO1: Wait,
    CTRL: OutputPin,
{
    reset: RESET,
    dio1: DIO1,
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<RESET, DIO1, CTRL> Stm32wbaLr1110InterfaceVariant<RESET, DIO1, CTRL>
where
    RESET: OutputPin,
    DIO1: Wait,
    CTRL: OutputPin,
{
    /// Create a new InterfaceVariant for LR1110 on STM32WBA
    ///
    /// # Arguments
    /// * `reset` - GPIO pin connected to LR1110 RESET
    /// * `dio1` - GPIO pin with EXTI connected to LR1110 DIO1 (IRQ)
    /// * `rf_switch_rx` - Optional GPIO for RX antenna switch
    /// * `rf_switch_tx` - Optional GPIO for TX antenna switch
    pub fn new(
        reset: RESET,
        dio1: DIO1,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        Ok(Self {
            reset,
            dio1,
            rf_switch_rx,
            rf_switch_tx,
        })
    }
}

impl<RESET, DIO1, CTRL> InterfaceVariant for Stm32wbaLr1110InterfaceVariant<RESET, DIO1, CTRL>
where
    RESET: OutputPin,
    DIO1: Wait,
    CTRL: OutputPin,
{
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        // LR1110 reset sequence:
        // 1. Set RESET low for at least 50us
        // 2. Wait 1ms
        // 3. Set RESET high
        // 4. Wait 5ms for chip to be ready
        delay.delay_ms(1).await;
        self.reset.set_low().map_err(|_| Reset)?;
        delay.delay_ms(10).await;
        self.reset.set_high().map_err(|_| Reset)?;
        delay.delay_ms(10).await;
        Ok(())
    }

    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        // LR1110 does not expose a BUSY pin in this configuration
        // The chip is ready quickly after commands
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
