#![allow(unused_macros)]
#![allow(unused)]

#[cfg(feature = "defmt")]
macro_rules! llog {
    (trace,   $($arg:expr),*) => { defmt::trace!($($arg),*) };
    (debug,   $($arg:expr),*) => { defmt::debug!($($arg),*) };
    (info,    $($arg:expr),*) => { defmt::info!($($arg),*) };
    (error,   $($arg:expr),*) => { defmt::error!($($arg),*) };
}

#[cfg(not(feature = "defmt"))]
macro_rules! llog {
    ($level:ident, $($arg:expr),*) => {{ $( let _ = $arg; )* }}
}
pub(crate) use llog;

macro_rules! trace {
    ($($arg:expr),*) => (log::llog!(trace, $($arg),*));
}
pub(crate) use trace;

macro_rules! debug {
    ($($arg:expr),*) => (log::llog!(debug, $($arg),*));
}
pub(crate) use debug;
macro_rules! info {
    ($($arg:expr),*) => (log::llog!(info, $($arg),*));
}
pub(crate) use info;

macro_rules! error {
    ($($arg:expr),*) => (log::llog!(error, $($arg),*));
}
pub(crate) use error;
