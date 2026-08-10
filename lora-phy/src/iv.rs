use embassy_futures::select::{Either, select};
use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;

use crate::mod_params::RadioError;
use crate::mod_params::RadioError::*;
use crate::mod_traits::InterfaceVariant;

/// Base for the InterfaceVariant implementation for the Sx127x.
///
/// The sx127x reports events on up to two DIO lines: DIO0 carries
/// RxDone/TxDone/CadDone, while RxTimeout is only ever routed to DIO1. LoRa
/// P2P and CAD only need DIO0, so [`new`](Self::new) takes a single IRQ pin.
///
/// [`lorawan_device::async_device::Device`], however, relies on RxTimeout to
/// close an empty receive window: with only DIO0 wired, an RX window that
/// hears nothing never wakes the driver and the join/receive hangs forever.
/// For LoRaWAN, construct with
/// [`new_with_secondary_irq`](Self::new_with_secondary_irq) so `await_irq`
/// also watches DIO1.
pub struct GenericSx127xInterfaceVariant<CTRL, WAIT> {
    reset: CTRL,
    irq: WAIT,
    irq_secondary: Option<WAIT>,
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<CTRL, WAIT> GenericSx127xInterfaceVariant<CTRL, WAIT>
where
    CTRL: OutputPin,
    WAIT: Wait,
{
    /// Create an InterfaceVariant watching a single IRQ pin (DIO0).
    ///
    /// Enough for LoRa P2P and CAD. Not enough for LoRaWAN receive windows,
    /// which need RxTimeout on DIO1; use
    /// [`new_with_secondary_irq`](Self::new_with_secondary_irq) there.
    pub fn new(
        reset: CTRL,
        irq: WAIT,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        Self::new_with_secondary_irq(reset, irq, None, rf_switch_rx, rf_switch_tx)
    }

    /// Create an InterfaceVariant watching DIO0 and, when supplied, DIO1.
    ///
    /// Pass the DIO1 pin as `irq_secondary` for LoRaWAN: the sx127x routes
    /// RxTimeout only to DIO1, so without it an empty receive window never
    /// completes. `await_irq` then wakes on whichever line fires and the
    /// driver reads RegIrqFlags to tell the events apart.
    pub fn new_with_secondary_irq(
        reset: CTRL,
        irq: WAIT,
        irq_secondary: Option<WAIT>,
        rf_switch_rx: Option<CTRL>,
        rf_switch_tx: Option<CTRL>,
    ) -> Result<Self, RadioError> {
        Ok(Self {
            reset,
            irq,
            irq_secondary,
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
        // Destructure so DIO0 and the optional DIO1 are borrowed as disjoint
        // fields; both futures must be live at once.
        let Self { irq, irq_secondary, .. } = self;
        match irq_secondary {
            None => irq.wait_for_high().await.map_err(|_| Irq)?,
            // Wake on either line. `select` polls DIO0 first, so a coincident
            // RxDone is reported ahead of RxTimeout.
            Some(dio1) => match select(irq.wait_for_high(), dio1.wait_for_high()).await {
                Either::First(r) | Either::Second(r) => r.map_err(|_| Irq)?,
            },
        }
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

/// Base for the InterfaceVariant implementation for Sx126x-based boards
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
    /// Create an InterfaceVariant instance for sx126x chips
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

/// Base for the InterfaceVariant implementation for LR1110/LR1120/LR1121-based boards
///
/// The LR11xx family uses a BUSY signal to indicate when the chip is processing
/// a command. The BUSY pin goes HIGH when the chip is busy and LOW when ready.
pub struct GenericLr1110InterfaceVariant<CTRL, WAIT> {
    reset: CTRL,
    dio1: WAIT,
    busy: WAIT,
    rf_switch_rx: Option<CTRL>,
    rf_switch_tx: Option<CTRL>,
}

impl<CTRL, WAIT> GenericLr1110InterfaceVariant<CTRL, WAIT>
where
    CTRL: OutputPin,
    WAIT: Wait,
{
    /// Create an InterfaceVariant instance for LR11xx chips
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

impl<CTRL, WAIT> InterfaceVariant for GenericLr1110InterfaceVariant<CTRL, WAIT>
where
    CTRL: OutputPin,
    WAIT: Wait,
{
    async fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), RadioError> {
        delay.delay_ms(10).await;
        self.reset.set_low().map_err(|_| Reset)?;
        delay.delay_ms(20).await;
        self.reset.set_high().map_err(|_| Reset)?;
        // Wait for chip to be ready after reset
        self.busy.wait_for_low().await.map_err(|_| Busy)?;
        Ok(())
    }

    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        // LR11xx BUSY pin is high when processing a command
        // Wait for it to go low before sending next command
        self.busy.wait_for_low().await.map_err(|_| Busy)
    }

    async fn await_irq(&mut self) -> Result<(), RadioError> {
        self.dio1.wait_for_high().await.map_err(|_| DIO1)?;
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
