/// LR1110 chip variant trait
///
/// This trait defines the interface for different LR1110 chip variants (LR1110, LR1120, LR1121)
/// to handle differences in PA selection, RF switch configuration, and other chip-specific features.
use super::radio_kind_params::{PaRegSupply, PaSelection};

/// Trait for LR1110 chip variants
///
/// Implement this trait to support different members of the LR11xx family with varying
/// power amplifier configurations and feature sets.
pub trait Lr1110Variant {
    /// Get the power amplifier selection for this variant
    ///
    /// Returns the PA type to use (LP, HP, or HF)
    fn get_pa_selection(&self) -> PaSelection;

    /// Whether to use DIO2 as RF switch control
    ///
    /// When true, DIO2 is automatically configured to control the RF switch.
    /// Default implementation returns true.
    fn use_dio2_as_rfswitch(&self) -> bool {
        true
    }

    /// Get the power amplifier supply source
    ///
    /// Returns whether the PA should be powered from the internal regulator (VREG)
    /// or directly from the battery (VBAT). VBAT is required for output power > +10dBm.
    /// Default implementation returns VBAT.
    fn get_pa_supply(&self) -> PaRegSupply {
        PaRegSupply::Vbat
    }
}

/// Standard LR1110 chip variant
///
/// This is the base LR1110 chip with all three power amplifiers available:
/// - LP: Low-power PA (up to +14dBm)
/// - HP: High-power PA (up to +22dBm)
/// - HF: High-frequency PA (2.4GHz band)
///
/// By default, uses the High-power PA for maximum output power.
pub struct Lr1110 {
    /// PA selection for this instance
    pub pa_selection: PaSelection,
}

impl Lr1110 {
    /// Create a new LR1110 instance with default HP PA
    pub const fn new() -> Self {
        Self {
            pa_selection: PaSelection::Hp,
        }
    }

    /// Create a new LR1110 instance with specific PA selection
    pub const fn with_pa(pa_selection: PaSelection) -> Self {
        Self { pa_selection }
    }
}

impl Default for Lr1110 {
    fn default() -> Self {
        Self::new()
    }
}

impl Lr1110Variant for Lr1110 {
    fn get_pa_selection(&self) -> PaSelection {
        self.pa_selection
    }

    fn get_pa_supply(&self) -> PaRegSupply {
        // Per LR1110 User Manual Table 9-1 and 9-2:
        // - LP PA uses Vreg (internal regulator)
        // - HP PA uses Vbat (battery)
        match self.pa_selection {
            PaSelection::Lp => PaRegSupply::Vreg,
            PaSelection::Hp | PaSelection::Hf => PaRegSupply::Vbat,
        }
    }
}

/// LR1120 chip variant (placeholder for future implementation)
///
/// The LR1120 is similar to LR1110 but may have different default configurations
/// or available features. Currently uses same implementation as LR1110.
pub struct Lr1120 {
    pub pa_selection: PaSelection,
}

impl Lr1120 {
    pub const fn new() -> Self {
        Self {
            pa_selection: PaSelection::Hp,
        }
    }

    pub const fn with_pa(pa_selection: PaSelection) -> Self {
        Self { pa_selection }
    }
}

impl Default for Lr1120 {
    fn default() -> Self {
        Self::new()
    }
}

impl Lr1110Variant for Lr1120 {
    fn get_pa_selection(&self) -> PaSelection {
        self.pa_selection
    }
}

/// LR1121 chip variant (placeholder for future implementation)
///
/// The LR1121 is the newest member of the LR11xx family and may have additional
/// features or different PA configurations. Currently uses same implementation as LR1110.
pub struct Lr1121 {
    pub pa_selection: PaSelection,
}

impl Lr1121 {
    pub const fn new() -> Self {
        Self {
            pa_selection: PaSelection::Hp,
        }
    }

    pub const fn with_pa(pa_selection: PaSelection) -> Self {
        Self { pa_selection }
    }
}

impl Default for Lr1121 {
    fn default() -> Self {
        Self::new()
    }
}

impl Lr1110Variant for Lr1121 {
    fn get_pa_selection(&self) -> PaSelection {
        self.pa_selection
    }
}
