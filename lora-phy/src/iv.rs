use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;

use crate::mod_params::RadioError;
use crate::mod_params::RadioError::*;
use crate::mod_traits::InterfaceVariant;

/// Base for the InterfaceVariant implementation for the Sx127x combination
pub struct GenericSx127xInterfaceVariant<CTRL, WAIT> {
    reset: CTRL,
    irq: WAIT,
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<CTRL, WAIT> GenericSx127xInterfaceVariant<CTRL, WAIT>
where
    CTRL: OutputPin,
    WAIT: Wait,
{
    /// Create an InterfaceVariant instance for an stm32l0/sx1276 combination
    pub fn new(
        reset: CTRL,
        irq: WAIT,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        Ok(Self {
            reset,
            irq,
            rf_switch_rx,
            rf_switch_tx,
        })
    }
}

impl<CTRL, WAIT> InterfaceVariant for GenericSx127xInterfaceVariant<CTRL, WAIT>
where
    CTRL: OutputPin,
    WAIT: Wait,
{
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        delay.delay_ms(10).await;
        self.reset.set_low().map_err(|_| Reset)?;
        delay.delay_ms(10).await;
        self.reset.set_high().map_err(|_| Reset)?;
        delay.delay_ms(10).await;
        Ok(())
    }
    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
    async fn await_irq(&mut self) -> Result<(), RadioError> {
        self.irq.wait_for_high().await.map_err(|_| Irq)
    }

    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError> {
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchTx)?,
            None => (),
        };
        match &mut self.rf_switch_rx {
            Some(pin) => pin.set_high().map_err(|_| RfSwitchRx),
            None => Ok(()),
        }
    }
    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError> {
        match &mut self.rf_switch_rx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchRx)?,
            None => (),
        };
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_high().map_err(|_| RfSwitchTx),
            None => Ok(()),
        }
    }
    async fn disable_rf_switch(&mut self) -> Result<(), RadioError> {
        match &mut self.rf_switch_rx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchRx)?,
            None => (),
        };
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchTx),
            None => Ok(()),
        }
    }
}

/// Base for the InterfaceVariant implementation for a generic Sx126x LoRa board
pub struct GenericSx126xInterfaceVariant<CTRL, WAIT> {
    reset: CTRL,
    dio1: WAIT,
    busy: WAIT,
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<CTRL, WAIT> GenericSx126xInterfaceVariant<CTRL, WAIT>
where
    CTRL: OutputPin,
    WAIT: Wait,
{
    /// Create an InterfaceVariant instance for an nrf52840/sx1262 combination
    pub fn new(
        reset: CTRL,
        dio1: WAIT,
        busy: WAIT,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        Ok(Self {
            reset,
            dio1,
            busy,
            rf_switch_rx,
            rf_switch_tx,
        })
    }
}

impl<CTRL, WAIT> InterfaceVariant for GenericSx126xInterfaceVariant<CTRL, WAIT>
where
    CTRL: OutputPin,
    WAIT: Wait,
{
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        delay.delay_ms(10).await;
        self.reset.set_low().map_err(|_| Reset)?;
        delay.delay_ms(20).await;
        self.reset.set_high().map_err(|_| Reset)?;
        delay.delay_ms(10).await;
        Ok(())
    }
    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        self.busy.wait_for_low().await.map_err(|_| Busy)
    }
    async fn await_irq(&mut self) -> Result<(), RadioError> {
        self.dio1.wait_for_high().await.map_err(|_| DIO1)?;
        Ok(())
    }

    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError> {
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchTx)?,
            None => (),
        };
        match &mut self.rf_switch_rx {
            Some(pin) => pin.set_high().map_err(|_| RfSwitchRx),
            None => Ok(()),
        }
    }
    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError> {
        match &mut self.rf_switch_rx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchRx)?,
            None => (),
        };
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_high().map_err(|_| RfSwitchTx),
            None => Ok(()),
        }
    }
    async fn disable_rf_switch(&mut self) -> Result<(), RadioError> {
        match &mut self.rf_switch_rx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchRx)?,
            None => (),
        };
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchTx),
            None => Ok(()),
        }
    }
}
