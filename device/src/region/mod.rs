#![allow(dead_code)]

#[cfg(feature = "region+us915")]
mod us915;

#[cfg(feature = "region+us915")]
pub use us915::Configuration as Region;

#[cfg(feature = "region+eu868")]
mod eu868;

#[cfg(feature = "region+eu868")]
pub use eu868::Configuration as Region;
