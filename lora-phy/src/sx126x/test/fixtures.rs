use crate::mod_params::RadioError;
use crate::mod_traits::InterfaceVariant;
use crate::sx126x::Sx126xVariant::Sx1261;
use crate::sx126x::{Config, Sx126x};
use embedded_hal::spi::{ErrorKind, Operation};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::SpiDevice;
use smtc_modem_cores::sx126x::{sx126x_status_e, Context, Status};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestFixture {
    pub ops: Vec<Ops>,
}

impl TestFixture {
    pub fn new() -> Self {
        Self { ops: vec![] }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ops {
    //Read(usize, Vec<u8>),
    Write(Vec<u8>),
}

#[no_mangle]
pub extern "C" fn sx126x_hal_write(
    context: *const core::ffi::c_void,
    command: *const u8,
    command_length: u16,
    data: *const u8,
    data_length: u16,
) -> sx126x_status_e {
    let context = unsafe { &mut *(context as *mut Context<TestFixture>) };
    let command = unsafe { std::slice::from_raw_parts(command, command_length as usize) };
    let data = unsafe { std::slice::from_raw_parts(data, data_length as usize) };
    let mut vec = command.to_vec();
    vec.extend(data.to_vec());
    context.inner.ops.push(Ops::Write(vec));
    Status::Ok.into()
}

#[derive(Debug)]
pub enum Error {}
impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> ErrorKind {
        todo!()
    }
}
impl embedded_hal::spi::ErrorType for TestFixture {
    type Error = Error;
}

impl SpiDevice<u8> for TestFixture {
    async fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.ops.push(Ops::Write(buf.to_vec()));
        Ok(())
    }

    async fn read(&mut self, _buf: &mut [u8]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transaction(&mut self, _operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transfer(&mut self, _read: &mut [u8], _write: &[u8]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transfer_in_place(&mut self, _buf: &mut [u8]) -> Result<(), Self::Error> {
        todo!()
    }
}

pub fn get_sx126x() -> Sx126x<TestFixture, DummyVariant> {
    Sx126x::new(
        TestFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1261,
            tcxo_ctrl: None,
            use_dcdc: false,
            use_dio2_as_rfswitch: false,
            rx_boost: true,
        },
    )
}

pub struct Delayer;
impl DelayNs for Delayer {
    async fn delay_ns(&mut self, _ns: u32) {}
}

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
