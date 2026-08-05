use super::DeviceSel;

/// One row of a PA power table: the SetPaConfig values for a band of output
/// powers, anchored at the row's highest power. Requests below the anchor
/// interpolate by lowering SetTxParams one-for-one, per the datasheet's
/// guidance for powers between the optimal settings.
#[derive(Clone, Copy, Debug)]
pub struct PaTableEntry {
    /// Highest output power this row covers [dBm]
    pub max_dbm: i8,
    /// SetPaConfig paDutyCycle for this row
    pub pa_duty_cycle: u8,
    /// SetPaConfig hpMax for this row (0 on the low-power PA)
    pub hp_max: u8,
    /// SetTxParams power commanded at `max_dbm`; lower targets within the
    /// row subtract the shortfall
    pub tx_params_at_max: i8,
}

/// A PA power table: rows sorted by ascending `max_dbm`; requests clamp to
/// `[min_dbm, last row's max_dbm]`. `&'static` tables live in flash — no RAM.
#[derive(Debug)]
pub struct PaTable {
    /// Lowest supported output power [dBm]
    pub min_dbm: i8,
    /// Rows sorted by ascending `max_dbm`; must be non-empty
    pub entries: &'static [PaTableEntry],
}

impl PaTable {
    /// The row covering `dbm` (clamped) and the SetTxParams power for it
    pub(crate) fn lookup(&self, dbm: i32) -> (&PaTableEntry, u8) {
        let max = self.entries[self.entries.len() - 1].max_dbm;
        let txp = dbm.clamp(self.min_dbm as i32, max as i32) as i8;
        let entry = self
            .entries
            .iter()
            .find(|e| e.max_dbm >= txp)
            .unwrap_or(&self.entries[self.entries.len() - 1]);
        (entry, (entry.tx_params_at_max - (entry.max_dbm - txp)) as u8)
    }
}

/// SX1261 low-power PA rows from datasheet Table 13-21 (+15/+14/+10 dBm)
pub const SX1261_PA_TABLE: PaTable = PaTable {
    min_dbm: -17,
    entries: &[
        PaTableEntry {
            max_dbm: 10,
            pa_duty_cycle: 0x01,
            hp_max: 0x00,
            tx_params_at_max: 13,
        },
        PaTableEntry {
            max_dbm: 14,
            pa_duty_cycle: 0x04,
            hp_max: 0x00,
            tx_params_at_max: 14,
        },
        PaTableEntry {
            max_dbm: 15,
            pa_duty_cycle: 0x06,
            hp_max: 0x00,
            tx_params_at_max: 14,
        },
    ],
};

/// SX1262 high-power PA rows from datasheet Table 13-21 (+22/+20/+17/+14 dBm);
/// every row commands SetTxParams +22 at its anchor
pub const SX1262_PA_TABLE: PaTable = PaTable {
    min_dbm: -9,
    entries: &[
        PaTableEntry {
            max_dbm: 14,
            pa_duty_cycle: 0x02,
            hp_max: 0x02,
            tx_params_at_max: 22,
        },
        PaTableEntry {
            max_dbm: 17,
            pa_duty_cycle: 0x02,
            hp_max: 0x03,
            tx_params_at_max: 22,
        },
        PaTableEntry {
            max_dbm: 20,
            pa_duty_cycle: 0x03,
            hp_max: 0x05,
            tx_params_at_max: 22,
        },
        PaTableEntry {
            max_dbm: 22,
            pa_duty_cycle: 0x04,
            hp_max: 0x07,
            tx_params_at_max: 22,
        },
    ],
};

/// ST's high-power table for the STM32WL (an SX126x die integrated into
/// ST's package, characterized by ST in STM32CubeWL). Identical to
/// [`SX1262_PA_TABLE`] except the 14 dBm-and-below row, where ST commands
/// the target dBm directly instead of interpolating from +22.
/// <https://github.com/STMicroelectronics/STM32CubeWL/blob/139e8d28bcec6af78dec8b52a9b9f9057868cc2e/Middlewares/Third_Party/SubGHz_Phy/stm32_radio_driver/radio_driver.c#L675>
pub const STM32WL_HP_PA_TABLE: PaTable = PaTable {
    min_dbm: -9,
    entries: &[
        PaTableEntry {
            max_dbm: 14,
            pa_duty_cycle: 0x02,
            hp_max: 0x02,
            tx_params_at_max: 14,
        },
        PaTableEntry {
            max_dbm: 17,
            pa_duty_cycle: 0x02,
            hp_max: 0x03,
            tx_params_at_max: 22,
        },
        PaTableEntry {
            max_dbm: 20,
            pa_duty_cycle: 0x03,
            hp_max: 0x05,
            tx_params_at_max: 22,
        },
        PaTableEntry {
            max_dbm: 22,
            pa_duty_cycle: 0x04,
            hp_max: 0x07,
            tx_params_at_max: 22,
        },
    ],
};

/// Implement this trait on your custom variant or use provided impls
pub trait Sx126xVariant {
    /// whether to use high or low power PA
    fn get_device_sel(&self) -> DeviceSel;

    /// use dio2 as rf switch output
    fn use_dio2_as_rfswitch(&self) -> bool {
        true
    }

    /// The PA power table for this variant. Defaults to the SX126x datasheet
    /// tables (Table 13-21) for the selected PA. Boards with their own PA
    /// characterization (e.g. the STM32WL) return a different `&'static`
    /// table — compile-time data, no RAM cost.
    fn pa_table(&self) -> &'static PaTable {
        match self.get_device_sel() {
            DeviceSel::LowPowerPA => &SX1261_PA_TABLE,
            DeviceSel::HighPowerPA => &SX1262_PA_TABLE,
        }
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
    fn pa_table(&self) -> &'static PaTable {
        if self.use_high_power_pa {
            &STM32WL_HP_PA_TABLE
        } else {
            &SX1261_PA_TABLE
        }
    }
}
