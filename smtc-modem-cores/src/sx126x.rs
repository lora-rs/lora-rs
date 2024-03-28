use smtc_modem_cores_sys::{
    sx126x_set_sleep, sx126x_sleep_cfgs_e_SX126X_SLEEP_CFG_COLD_START,
    sx126x_sleep_cfgs_e_SX126X_SLEEP_CFG_WARM_START, sx126x_status_e_SX126X_STATUS_ERROR,
    sx126x_status_e_SX126X_STATUS_OK, sx126x_status_e_SX126X_STATUS_UNKNOWN_VALUE,
    sx126x_status_e_SX126X_STATUS_UNSUPPORTED_FEATURE, sx126x_status_t,
};
use std::convert::TryFrom;
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

impl TryFrom<u32> for Status {
    type Error = Error;
    fn try_from(value: u32) -> Result<Status> {
        match value {
            sx126x_status_e_SX126X_STATUS_OK => Ok(Status::Ok),
            sx126x_status_e_SX126X_STATUS_UNSUPPORTED_FEATURE => Ok(Status::UnSupportedFeature),
            sx126x_status_e_SX126X_STATUS_UNKNOWN_VALUE => Ok(Status::UnknownValue),
            sx126x_status_e_SX126X_STATUS_ERROR => Ok(Status::Error),
            v => Err(Error::InvalidStatusValue(v)),
        }
    }
}

impl From<Status> for u32 {
    fn from(status: Status) -> u32 {
        match status {
            Status::Ok => sx126x_status_e_SX126X_STATUS_OK,
            Status::UnSupportedFeature => sx126x_status_e_SX126X_STATUS_UNSUPPORTED_FEATURE,
            Status::UnknownValue => sx126x_status_e_SX126X_STATUS_UNKNOWN_VALUE,
            Status::Error => sx126x_status_e_SX126X_STATUS_ERROR,
        }
    }
}

pub enum SleepCfg {
    ColdStart,
    WarmStart,
}

impl From<SleepCfg> for u32 {
    fn from(cfg: SleepCfg) -> u32 {
        match cfg {
            SleepCfg::ColdStart => sx126x_sleep_cfgs_e_SX126X_SLEEP_CFG_COLD_START,
            SleepCfg::WarmStart => sx126x_sleep_cfgs_e_SX126X_SLEEP_CFG_WARM_START,
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
    context.spi.write(slice).unwrap();
    let slice = unsafe { std::slice::from_raw_parts(data, data_length as usize) };
    context.spi.write(slice).unwrap();
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
