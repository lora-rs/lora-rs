use embedded_hal_02::digital::v2::OutputPin;

pub struct RFSwitch<CTRL> {
    rx_gpio: Option<CTRL>,
    tx_gpio: Option<CTRL>,
}

impl <CTRL> RFSwitch<CTRL>
where
    CTRL: OutputPin,
{
    pub fn new(
        rx_gpio: Option<CTRL>,
        tx_gpio: Option<CTRL>,
    ) -> Self {
        Self {
            rx_gpio,
            tx_gpio,
        }
    }

    fn enable_rx(&mut self) -> Result<(), <CTRL>::Error> {
        match &mut self.tx_gpio {
            Some(pin) => pin.set_low()?,
            None => (),
        };
        match &mut self.rx_gpio {
            Some(pin) => pin.set_high(),
            None => Ok(()),
        }
    }
    fn enable_tx(&mut self) -> Result<(), <CTRL>::Error> {
        match &mut self.rx_gpio {
            Some(pin) => pin.set_low()?,
            None => (),
        };
        match &mut self.tx_gpio {
            Some(pin) => pin.set_high(),
            None => Ok(()),
        }
    }
    fn disable(&mut self) -> Result<(), <CTRL>::Error> {
        match &mut self.rx_gpio {
            Some(pin) => pin.set_low()?,
            None => (),
        };
        match &mut self.tx_gpio {
            Some(pin) => pin.set_low(),
            None => Ok(()),
        }
    }
}
