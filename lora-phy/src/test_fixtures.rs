//! Chip-independent scaffolding shared by the per-chip comparison-test
//! fixtures. Only zero-semantics boilerplate belongs here — each fixture's
//! transaction recording and equality rules are chip-specific by design.
use crate::mod_params::RadioError;
use crate::mod_traits::InterfaceVariant;
use embedded_hal::spi::ErrorKind;
use embedded_hal_async::delay::DelayNs;

/// SPI error type for fixtures that never fail
#[derive(Debug)]
pub enum SpiError {}
impl embedded_hal::spi::Error for SpiError {
    fn kind(&self) -> ErrorKind {
        match *self {}
    }
}

/// Delay provider that returns immediately
pub struct Delayer;
impl DelayNs for Delayer {
    async fn delay_ns(&mut self, _ns: u32) {}
}

/// InterfaceVariant with no-op control lines (busy always ready, IRQ
/// always pending, RF switch ignored)
pub struct DummyVariant;

impl InterfaceVariant for DummyVariant {
    async fn reset(&mut self, _delay: &mut impl DelayNs) -> Result<(), RadioError> {
        Ok(())
    }
    async fn wait_on_busy(&mut self) -> Result<(), RadioError> {
        Ok(())
    }
    async fn await_irq(&mut self) -> Result<(), RadioError> {
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
