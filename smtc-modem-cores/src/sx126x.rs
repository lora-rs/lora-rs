use smtc_modem_cores_sys::{
    sx126x_set_sleep, sx126x_sleep_cfgs_e, sx126x_status_e, sx126x_status_t,
};
use std::ffi;

pub struct Context<S: embedded_hal::spi::SpiDevice> {
    spi: S,
}

pub enum Status {
    Ok,
    UnSupportedFeature,
    UnknownValue,
    Error,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("smtc-modem-cores sx126x invalid status value: {0}")]
    InvalidStatusValue(u32),
}

pub type Result<T = ()> = std::result::Result<T, Error>;

impl From<sx126x_status_e> for Status {
    fn from(value: sx126x_status_e) -> Status {
        match value {
            sx126x_status_e::SX126X_STATUS_OK => Status::Ok,
            sx126x_status_e::SX126X_STATUS_UNSUPPORTED_FEATURE => Status::UnSupportedFeature,
            sx126x_status_e::SX126X_STATUS_UNKNOWN_VALUE => Status::UnknownValue,
            sx126x_status_e::SX126X_STATUS_ERROR => Status::Error,
        }
    }
}

impl From<Status> for sx126x_status_e {
    fn from(status: Status) -> sx126x_status_e {
        match status {
            Status::Ok => sx126x_status_e::SX126X_STATUS_OK,
            Status::UnSupportedFeature => sx126x_status_e::SX126X_STATUS_UNSUPPORTED_FEATURE,
            Status::UnknownValue => sx126x_status_e::SX126X_STATUS_UNKNOWN_VALUE,
            Status::Error => sx126x_status_e::SX126X_STATUS_ERROR,
        }
    }
}

pub enum SleepCfg {
    ColdStart,
    WarmStart,
}

impl From<SleepCfg> for sx126x_sleep_cfgs_e {
    fn from(cfg: SleepCfg) -> sx126x_sleep_cfgs_e {
        match cfg {
            SleepCfg::ColdStart => sx126x_sleep_cfgs_e::SX126X_SLEEP_CFG_COLD_START,
            SleepCfg::WarmStart => sx126x_sleep_cfgs_e::SX126X_SLEEP_CFG_WARM_START,
        }
    }
}

impl<S: embedded_hal::spi::SpiDevice> Context<S> {
    pub fn new(spi: S) -> Self {
        Self { spi }
    }

    pub fn set_sleep(&self, cfg: SleepCfg) {
        let ptr = self as *const Self as *const ffi::c_void;
        unsafe {
            sx126x_set_sleep(ptr, cfg.into());
        }
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn sx126x_hal_write_generic<S: embedded_hal::spi::SpiDevice>(
    context: *const ffi::c_void,
    command: *const u8,
    command_length: u16,
    data: *const u8,
    data_length: u16,
) -> sx126x_status_t {
    let context = unsafe { &mut *(context as *mut Context<S>) };
    let slice = unsafe { std::slice::from_raw_parts(command, command_length as usize) };
    if context.spi.write(slice).is_err() {
        return sx126x_status_t::SX126X_STATUS_ERROR;
    }
    let slice = unsafe { std::slice::from_raw_parts(data, data_length as usize) };
    if context.spi.write(slice).is_err() {
        return sx126x_status_t::SX126X_STATUS_ERROR;
    }
    Status::Ok.into()
}

/// While we implement the sx126x_hal_write_generic over a generic SpiDevice, the user must provide
/// the concrete implementation for the compiler to link to the C code.
#[macro_export]
macro_rules! concrete_export {
    ($s:tt) => {
        #[no_mangle]
        pub extern "C" fn sx126x_hal_write(
            context: *const ffi::c_void,
            command: *const u8,
            command_length: u16,
            data: *const u8,
            data_length: u16,
        ) -> sx126x_status_t {
            unsafe {
                sx126x_hal_write_generic::<$s>(context, command, command_length, data, data_length)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::spi::{ErrorKind, Operation};
    use std::result::Result;

    struct TestSpi {
        data: Vec<u8>,
    }

    #[derive(Debug)]
    enum Error {}
    impl embedded_hal::spi::Error for Error {
        fn kind(&self) -> ErrorKind {
            todo!()
        }
    }
    impl embedded_hal::spi::ErrorType for TestSpi {
        type Error = Error;
    }
    impl embedded_hal::spi::SpiDevice for TestSpi {
        fn transaction(
            &mut self,
            _operations: &mut [Operation<'_, u8>],
        ) -> Result<(), Self::Error> {
            todo!()
        }

        fn read(&mut self, data: &mut [u8]) -> Result<(), Self::Error> {
            self.data.copy_from_slice(data);
            Ok(())
        }

        fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            self.data.extend_from_slice(data);
            Ok(())
        }
        fn transfer(&mut self, _read: &mut [u8], _write: &[u8]) -> Result<(), Self::Error> {
            todo!()
        }
        fn transfer_in_place(&mut self, _buf: &mut [u8]) -> Result<(), Self::Error> {
            todo!()
        }
    }

    concrete_export!(TestSpi);

    #[test]
    fn sleep() {
        let spi = TestSpi { data: vec![] };
        let context = Context::new(spi);
        context.set_sleep(SleepCfg::ColdStart);
    }
}
