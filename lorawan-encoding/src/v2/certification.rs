//! LoRaWAN Certification Protocol (TS009) parsing with explicit errors.
//!
//! The command enums, payload accessors and creators are shared with
//! [`crate::certification`]; this module provides the v2 framing layer
//! (see [`super::mac`]) for them. Unknown CIDs and truncated payloads are
//! reported instead of silently ending the stream, and a variable-length
//! command with an empty payload (e.g. a lone `EchoIncPayloadReq` CID) is
//! `Truncated` instead of yielding a bogus empty command.

pub use crate::certification::{DownlinkDUTCommand, UplinkDUTCommand};

use super::mac::MacCommands;

/// Parses a stream of downlink (server-transmitted) DUT commands.
#[inline]
pub fn parse_downlink_dut_commands(data: &[u8]) -> MacCommands<'_, DownlinkDUTCommand<'_>> {
    MacCommands::new(data)
}

/// Parses a stream of uplink (device-transmitted) DUT commands.
#[inline]
pub fn parse_uplink_dut_commands(data: &[u8]) -> MacCommands<'_, UplinkDUTCommand<'_>> {
    MacCommands::new(data)
}
