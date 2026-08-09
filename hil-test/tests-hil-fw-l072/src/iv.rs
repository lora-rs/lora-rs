//! InterfaceVariant for the Murata CMWX1ZZABZ (SX1276) awaiting DIO0 *and*
//! DIO1. lora-phy's `GenericSx127xInterfaceVariant` watches a single pin,
//! but the sx127x driver maps RxTimeout to DIO1 (RxDone/TxDone to DIO0), so
//! with one pin an empty RX-single window never wakes and the device hangs.
//! On this module DIO0 = PB4 and DIO1 = PB1.

use embassy_futures::select::select;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::mode::Async;
use embedded_hal::digital::OutputPin;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_params::RadioError::*;
use lora_phy::mod_traits::InterfaceVariant;
use lora_phy::DelayNs;

pub struct MurataInterfaceVariant<CTRL> {
    reset: CTRL,
    dio0: ExtiInput<'static, Async>,
    dio1: ExtiInput<'static, Async>,
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<CTRL> MurataInterfaceVariant<CTRL>
where
    CTRL: OutputPin,
{
    pub fn new(
        reset: CTRL,
        dio0: ExtiInput<'static, Async>,
        dio1: ExtiInput<'static, Async>,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        Ok(Self {
            reset,
            dio0,
            dio1,
            rf_switch_rx,
            rf_switch_tx,
        })
    }
}

impl<CTRL> InterfaceVariant for MurataInterfaceVariant<CTRL>
where
    CTRL: OutputPin,
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
        select(self.dio0.wait_for_high(), self.dio1.wait_for_high()).await;
        Ok(())
    }

    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError> {
        if let Some(pin) = &mut self.rf_switch_tx {
            pin.set_low().map_err(|_| RfSwitchTx)?
        };
        match &mut self.rf_switch_rx {
            Some(pin) => pin.set_high().map_err(|_| RfSwitchRx),
            None => Ok(()),
        }
    }

    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError> {
        if let Some(pin) = &mut self.rf_switch_rx {
            pin.set_low().map_err(|_| RfSwitchRx)?
        };
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_high().map_err(|_| RfSwitchTx),
            None => Ok(()),
        }
    }

    async fn disable_rf_switch(&mut self) -> Result<(), RadioError> {
        if let Some(pin) = &mut self.rf_switch_rx {
            pin.set_low().map_err(|_| RfSwitchRx)?
        };
        match &mut self.rf_switch_tx {
            Some(pin) => pin.set_low().map_err(|_| RfSwitchTx),
            None => Ok(()),
        }
    }
}
