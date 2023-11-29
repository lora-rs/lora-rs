use embassy_stm32::interrupt;
use embassy_stm32::interrupt::InterruptExt;
use embassy_stm32::pac;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::{ErrorType, Operation, SpiBus, SpiDevice};
use lora_phy::mod_params::RadioError;
use lora_phy::mod_params::RadioError::*;
use lora_phy::mod_traits::InterfaceVariant;
use lora_phy::DelayNs;

/// Interrupt handler.
pub struct InterruptHandler {}

impl interrupt::typelevel::Handler<interrupt::typelevel::SUBGHZ_RADIO> for InterruptHandler {
    unsafe fn on_interrupt() {
        interrupt::SUBGHZ_RADIO.disable();
        IRQ_SIGNAL.signal(());
    }
}

static IRQ_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Base for the InterfaceVariant implementation for an stm32wl/sx1262 combination
pub struct Stm32wlInterfaceVariant<CTRL> {
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<CTRL> Stm32wlInterfaceVariant<CTRL>
where
    CTRL: OutputPin,
{
    /// Create an InterfaceVariant instance for an stm32wl/sx1262 combination
    pub fn new(
        _irq: impl interrupt::typelevel::Binding<interrupt::typelevel::SUBGHZ_RADIO, InterruptHandler> + 'static,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        interrupt::SUBGHZ_RADIO.disable();
        Ok(Self {
            rf_switch_rx,
            rf_switch_tx,
        })
    }
}

impl<CTRL> InterfaceVariant for Stm32wlInterfaceVariant<CTRL>
where
    CTRL: OutputPin,
{
    async fn reset(&mut self, _delay: &mut impl DelayNs) -> Result<(), RadioError> {
        pac::RCC.csr().modify(|w| w.set_rfrst(true));
        pac::RCC.csr().modify(|w| w.set_rfrst(false));
        Ok(())
    }
    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        while pac::PWR.sr2().read().rfbusys() {}
        Ok(())
    }

    async fn await_irq(&mut self) -> Result<(), RadioError> {
        unsafe { interrupt::SUBGHZ_RADIO.enable() };
        IRQ_SIGNAL.wait().await;
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

pub struct SubghzSpiDevice<T>(pub T);

impl<T: SpiBus> ErrorType for SubghzSpiDevice<T> {
    type Error = T::Error;
}

impl<T: SpiBus> SpiDevice for SubghzSpiDevice<T> {
    async fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        pac::PWR.subghzspicr().modify(|w| w.set_nss(false));

        let op_res = 'ops: {
            for op in operations {
                let res = match op {
                    Operation::Read(buf) => self.0.read(buf).await,
                    Operation::Write(buf) => self.0.write(buf).await,
                    Operation::Transfer(read, write) => self.0.transfer(read, write).await,
                    Operation::TransferInPlace(buf) => self.0.transfer_in_place(buf).await,
                    Operation::DelayNs(ns) => match self.0.flush().await {
                        Err(e) => Err(e),
                        Ok(()) => {
                            Timer::after_nanos((*ns) as u64).await;
                            Ok(())
                        }
                    },
                };
                if let Err(e) = res {
                    break 'ops Err(e);
                }
            }
            Ok(())
        };

        // On failure, it's important to still flush and deassert CS.
        let flush_res = self.0.flush().await;

        pac::PWR.subghzspicr().modify(|w| w.set_nss(true));

        op_res?;
        flush_res?;

        Ok(())
    }
}
