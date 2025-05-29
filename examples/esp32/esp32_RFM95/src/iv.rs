use embassy_futures::select::{select, Either};
use embedded_hal::digital::OutputPin;
use embedded_hal_async::{
    digital::Wait,
    delay::DelayNs,
};
use lora_phy::{
    mod_params::{
        RadioError,
        RadioError::*,
    },
    mod_traits::InterfaceVariant,
};
use defmt::info;

pub struct LoraWanSx127xInterfaceVariant<CTRL, WAIT> {
    reset: CTRL,
    dio0: WAIT,
    dio1: WAIT
}

impl<CTRL, WAIT> LoraWanSx127xInterfaceVariant<CTRL, WAIT> {
    pub fn new(
        reset: CTRL,
        dio0: WAIT,
        dio1: WAIT
    ) -> Result<Self, RadioError> {
        Ok(Self {
            reset,
            dio0,
            dio1,
        })
    }
}

impl<CTRL, WAIT> InterfaceVariant for LoraWanSx127xInterfaceVariant<CTRL, WAIT>
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

    async fn await_irq(&mut self) -> Result<(), RadioError> {
        match select(self.dio0.wait_for_high(), self.dio1.wait_for_high()).await {
            Either::First(_) => {
                info!("dio0");
            }
            Either::Second(_) => {
                info!("dio1");
            }
        }
        Ok(())
    }

    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        Ok(())
    }    

    async fn enable_rf_switch_rx(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn enable_rf_switch_tx(&mut self) -> Result<(), RadioError> {
        Ok(())
    }

    async fn disable_rf_switch(&mut self) -> Result<(), RadioError> {
        Ok(())
    }    
}
    


