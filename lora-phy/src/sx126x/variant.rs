use super::DeviceSel;

/// Implement this trait on your custom variant or use provided impls
pub trait Sx126xVariant {
    /// whether to use high or low power PA
    fn get_device_sel(&self) -> DeviceSel;

    /// use dio2 as rf switch output
    fn use_dio2_as_rfswitch(&self) -> bool {
        true
    }

    /// Use ST's power table instead of the SX126x datasheet's. The STM32WL
    /// is an SX126x die integrated into ST's package, characterized by ST
    /// with its own table; discrete Semtech parts take the datasheet table
    /// (Table 13-21). The two differ only in the high-power PA's 14 dBm-and-
    /// below row: same PA config, but ST commands the target dBm directly
    /// where the datasheet interpolates from a +22 setpoint.
    fn use_st_power_table(&self) -> bool {
        false
    }
}

/// Sx1261 uses only LowPowerPA
pub struct Sx1261;
impl Sx126xVariant for Sx1261 {
    fn get_device_sel(&self) -> super::DeviceSel {
        super::DeviceSel::LowPowerPA
    }
}

/// Sx1262 uses only HighPowerPA
pub struct Sx1262;

impl Sx126xVariant for Sx1262 {
    fn get_device_sel(&self) -> super::DeviceSel {
        super::DeviceSel::HighPowerPA
    }
}

/// Stm32wl variant.
pub struct Stm32wl {
    /// select which output to use. (Switching is not supported)
    pub use_high_power_pa: bool,
}
impl Sx126xVariant for Stm32wl {
    fn get_device_sel(&self) -> super::DeviceSel {
        if self.use_high_power_pa {
            DeviceSel::HighPowerPA
        } else {
            DeviceSel::LowPowerPA
        }
    }
    fn use_dio2_as_rfswitch(&self) -> bool {
        false
    }
    fn use_st_power_table(&self) -> bool {
        true
    }
}
