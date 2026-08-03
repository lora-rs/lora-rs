//! Remote Multicast Setup (TS005) parsing with explicit errors.
//!
//! The command enums, payload accessors, session-key derivation and creators
//! are shared with [`crate::multicast`]; this module provides the v2 framing
//! layer (see [`super::mac`]) for them.
//!
//! Notably, parsing a lone `McGroupStatusAns` CID byte through the legacy
//! iterator panics (its length callback indexes an empty payload); here it is
//! reported as `Truncated`.

pub use crate::multicast::{DownlinkRemoteSetup, Session, UplinkRemoteSetup, MAX_GROUPS};

use super::mac::MacCommands;

/// Parses a stream of downlink (server-transmitted) multicast setup commands.
#[inline]
pub fn parse_downlink_multicast_commands(data: &[u8]) -> MacCommands<'_, DownlinkRemoteSetup<'_>> {
    MacCommands::new(data)
}

/// Parses a stream of uplink (device-transmitted) multicast setup commands.
#[inline]
pub fn parse_uplink_multicast_commands(data: &[u8]) -> MacCommands<'_, UplinkRemoteSetup<'_>> {
    MacCommands::new(data)
}
