//! Non-volatile storage for LoRaWAN 1.0.4 operation.
//!
//! Providing a store to the device enables the 1.0.4 behavior: monotonic DevNonce,
//! JoinNonce replay protection and session resume across power loss. With the default
//! [`NoNvm`] the stack behaves as LoRaWAN 1.0.2 (random DevNonce, no persistence).
//!
//! The version is selected at compile time by the store type, via
//! [`NonVolatileStore::PERSISTENT`], so a build cannot claim 1.0.4 without somewhere to
//! keep its anti-replay state.
//!
//! Two regions are stored, split because their write rates differ by orders of magnitude:
//! - [`NvmRegion::Identity`]: DevNonce/JoinNonce counters. Written around joins only,
//!   never reset for the life of the device.
//! - [`NvmRegion::Session`]: session keys, frame counters and negotiated MAC state.
//!   Written on join, on accepted downlinks, and when the uplink counter reaches its
//!   checkpoint.
//!
//! Surviving power loss mid-write is the store implementation's responsibility (two-slot
//! ping-pong or a crate such as `sequential-storage`). Blobs carry a CRC-protected header,
//! so a torn write is detected and treated as an absent region. For the identity region
//! that fallback restarts the DevNonce at 0, which a join server rejects until the counter
//! passes its last recorded value, so identity writes in particular must be atomic.

use core::fmt::Debug;

use lorawan::keys::{AppSKey, NwkSKey};
use lorawan::parser::DevAddr;
use lorawan::types::DR;

/// Which logical slot is being read or written. The two regions have very different
/// write cadences; store implementations may want to place them accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum NvmRegion {
    /// Join anti-replay counters. Survives even a session wipe.
    Identity,
    /// Active session state. Discarded whenever it does not match the identity epoch.
    Session,
}

/// Largest encoded blob (header included) the stack will ever pass to `save` or expect
/// from `load`. Store implementations may size internal buffers with this.
pub const MAX_BLOB_LEN: usize = 96;

/// Default headroom persisted ahead of the live uplink frame counter. The `Session`
/// region is rewritten only when the counter reaches the checkpoint, and boot resumes
/// from the checkpoint, so up to this many values are skipped per power cycle instead
/// of any being reused. A 1.0.2 server allows a gap of 16384; 1.0.4 has no gap limit.
pub const DEFAULT_FCNT_CHECKPOINT_MARGIN: u32 = 32;

/// User-supplied non-volatile store. Blocking implementations should implement
/// [`NonVolatileStoreSync`] instead; a blanket impl makes every sync store usable here.
#[allow(async_fn_in_trait)]
pub trait NonVolatileStore {
    type Error: Debug;

    /// True for any real store; false only for [`NoNvm`]. Selects 1.0.2 or 1.0.4 at
    /// compile time.
    const PERSISTENT: bool = true;

    /// Durably store `bytes` as the current blob for `region`, replacing any previous
    /// blob. Must not return before the data is safe against power loss.
    async fn save(&mut self, region: NvmRegion, bytes: &[u8]) -> Result<(), Self::Error>;

    /// Read the current blob for `region` into `buf`, returning its length, or
    /// `Ok(None)` if the region has never been written.
    async fn load(
        &mut self,
        region: NvmRegion,
        buf: &mut [u8],
    ) -> Result<Option<usize>, Self::Error>;
}

/// Blocking sibling of [`NonVolatileStore`] with the same contract.
pub trait NonVolatileStoreSync {
    type Error: Debug;

    /// See [`NonVolatileStore::PERSISTENT`].
    const PERSISTENT: bool = true;

    /// See [`NonVolatileStore::save`].
    fn save(&mut self, region: NvmRegion, bytes: &[u8]) -> Result<(), Self::Error>;

    /// See [`NonVolatileStore::load`].
    fn load(&mut self, region: NvmRegion, buf: &mut [u8]) -> Result<Option<usize>, Self::Error>;
}

/// Every synchronous store is trivially an asynchronous one.
impl<T: NonVolatileStoreSync> NonVolatileStore for T {
    type Error = T::Error;
    const PERSISTENT: bool = T::PERSISTENT;

    async fn save(&mut self, region: NvmRegion, bytes: &[u8]) -> Result<(), Self::Error> {
        NonVolatileStoreSync::save(self, region, bytes)
    }

    async fn load(
        &mut self,
        region: NvmRegion,
        buf: &mut [u8],
    ) -> Result<Option<usize>, Self::Error> {
        NonVolatileStoreSync::load(self, region, buf)
    }
}

/// Error type of [`NoNvm`], never actually produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct Unsupported;

/// The default store: no persistence, LoRaWAN 1.0.2 behavior. All persistence paths
/// compile out.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoNvm;

impl NonVolatileStoreSync for NoNvm {
    type Error = Unsupported;
    const PERSISTENT: bool = false;

    fn save(&mut self, _region: NvmRegion, _bytes: &[u8]) -> Result<(), Unsupported> {
        Err(Unsupported)
    }

    fn load(&mut self, _region: NvmRegion, _buf: &mut [u8]) -> Result<Option<usize>, Unsupported> {
        Err(Unsupported)
    }
}

/// Durable join anti-replay state. Written around joins only, never reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct PersistentIdentity {
    /// 1.0.4 monotonic join counter; +1 per JoinRequest, never resets. `u16::MAX` marks
    /// exhaustion: no further join is attempted with this identity.
    pub dev_nonce: u16,
    /// JoinNonce of the last accepted JoinAccept; an accept carrying the same value is
    /// rejected as a replay. 1.0.4 only guarantees a non-repeating JoinNonce, so this is
    /// the strongest check that never rejects a compliant server. `None` until the first
    /// successful join.
    pub last_join_nonce: Option<u32>,
    /// Bumps on each successful join; ties a session blob to this identity.
    pub join_epoch: u32,
}

/// Durable session state, sufficient to resume sending after a cold boot. A dedicated
/// schema, independent of the in-RAM `Session`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentSession {
    pub nwkskey: NwkSKey,
    pub appskey: AppSKey,
    pub devaddr: DevAddr,
    /// Boot resumes `fcnt_up` from here; always ahead of any counter that went on air.
    pub fcnt_up_checkpoint: u32,
    /// Exact last-accepted downlink counter, `None` before the session's first downlink.
    pub fcnt_down: Option<u32>,
    /// Must equal `PersistentIdentity::join_epoch` or the session is stale.
    pub join_epoch: u32,
    // Negotiated MAC state. Channel-plan state is not persisted yet; adding it is a
    // schema version bump.
    pub data_rate: DR,
    pub rx1_delay: u32,
    pub rx1_dr_offset: u8,
    pub rx2_data_rate: Option<DR>,
    pub rx2_frequency: Option<u32>,
    pub tx_power: Option<u8>,
    pub adr_enabled: bool,
}

/// Reasons a stored blob was rejected. All are handled by treating the region as
/// absent; the variants exist for logging and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum DecodeError {
    /// Missing magic or header shorter than minimum: not one of our blobs.
    BadMagic,
    /// Blob was written by an incompatible firmware version.
    SchemaVersion,
    /// Blob belongs to the other region (slots swapped by the store).
    RegionMismatch,
    /// Length field disagrees with the buffer or the expected payload size.
    BadLength,
    /// Payload failed its integrity check (torn or corrupted write).
    BadCrc,
}

const MAGIC: [u8; 4] = *b"LWNV";
const HEADER_LEN: usize = 12;
const IDENTITY_SCHEMA_VERSION: u8 = 1;
const SESSION_SCHEMA_VERSION: u8 = 1;
const IDENTITY_PAYLOAD_LEN: usize = 10;
const SESSION_PAYLOAD_LEN: usize = 62;
/// Sentinel encoding of `last_join_nonce: None`; JoinNonce is a 3-byte field on the
/// wire, so this value can never be a real nonce.
const JOIN_NONCE_NONE: u32 = 0xFFFF_FFFF;

fn crc32(seed: u32, data: &[u8]) -> u32 {
    let mut crc = !seed;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn region_tag(region: NvmRegion) -> u8 {
    match region {
        NvmRegion::Identity => 0,
        NvmRegion::Session => 1,
    }
}

fn schema_version(region: NvmRegion) -> u8 {
    match region {
        NvmRegion::Identity => IDENTITY_SCHEMA_VERSION,
        NvmRegion::Session => SESSION_SCHEMA_VERSION,
    }
}

/// Header layout: `magic[4] | region_tag u8 | schema_version u8 | payload_len u16 LE |
/// crc32 u32 LE`, CRC computed over region_tag, schema_version, payload_len and payload.
fn encode_blob(region: NvmRegion, payload: &[u8], out: &mut [u8]) -> usize {
    let total = HEADER_LEN + payload.len();
    debug_assert!(out.len() >= total && payload.len() <= u16::MAX as usize);
    out[0..4].copy_from_slice(&MAGIC);
    out[4] = region_tag(region);
    out[5] = schema_version(region);
    out[6..8].copy_from_slice(&(payload.len() as u16).to_le_bytes());
    out[HEADER_LEN..total].copy_from_slice(payload);
    let crc = crc32(crc32(0, &out[4..8]), payload);
    out[8..12].copy_from_slice(&crc.to_le_bytes());
    total
}

fn decode_blob(region: NvmRegion, blob: &[u8]) -> Result<&[u8], DecodeError> {
    if blob.len() < HEADER_LEN || blob[0..4] != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    if blob[4] != region_tag(region) {
        return Err(DecodeError::RegionMismatch);
    }
    if blob[5] != schema_version(region) {
        return Err(DecodeError::SchemaVersion);
    }
    let len = u16::from_le_bytes([blob[6], blob[7]]) as usize;
    if blob.len() != HEADER_LEN + len {
        return Err(DecodeError::BadLength);
    }
    let crc = u32::from_le_bytes([blob[8], blob[9], blob[10], blob[11]]);
    if crc != crc32(crc32(0, &blob[4..8]), &blob[HEADER_LEN..]) {
        return Err(DecodeError::BadCrc);
    }
    Ok(&blob[HEADER_LEN..])
}

impl PersistentIdentity {
    /// Encode as a self-describing blob into `out` (sized at least [`MAX_BLOB_LEN`]),
    /// returning the encoded length.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        let mut p = [0u8; IDENTITY_PAYLOAD_LEN];
        p[0..2].copy_from_slice(&self.dev_nonce.to_le_bytes());
        p[2..6].copy_from_slice(&self.last_join_nonce.unwrap_or(JOIN_NONCE_NONE).to_le_bytes());
        p[6..10].copy_from_slice(&self.join_epoch.to_le_bytes());
        encode_blob(NvmRegion::Identity, &p, out)
    }

    pub fn decode(blob: &[u8]) -> Result<Self, DecodeError> {
        let p = decode_blob(NvmRegion::Identity, blob)?;
        if p.len() != IDENTITY_PAYLOAD_LEN {
            return Err(DecodeError::BadLength);
        }
        let last_join_nonce = u32::from_le_bytes([p[2], p[3], p[4], p[5]]);
        Ok(Self {
            dev_nonce: u16::from_le_bytes([p[0], p[1]]),
            last_join_nonce: (last_join_nonce != JOIN_NONCE_NONE).then_some(last_join_nonce),
            join_epoch: u32::from_le_bytes([p[6], p[7], p[8], p[9]]),
        })
    }
}

impl PersistentSession {
    /// Encode as a self-describing blob into `out` (sized at least [`MAX_BLOB_LEN`]),
    /// returning the encoded length.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        let mut p = [0u8; SESSION_PAYLOAD_LEN];
        p[0..16].copy_from_slice(&self.nwkskey.inner().0);
        p[16..32].copy_from_slice(&self.appskey.inner().0);
        p[32..36].copy_from_slice(self.devaddr.as_wire_bytes());
        p[36..40].copy_from_slice(&self.fcnt_up_checkpoint.to_le_bytes());
        p[40..44].copy_from_slice(&self.fcnt_down.unwrap_or(0).to_le_bytes());
        p[44..48].copy_from_slice(&self.join_epoch.to_le_bytes());
        p[48] = self.data_rate as u8;
        p[49..53].copy_from_slice(&self.rx1_delay.to_le_bytes());
        p[53] = self.rx1_dr_offset;
        p[54] = self.rx2_data_rate.map_or(0xFF, |dr| dr as u8);
        p[55..59].copy_from_slice(&self.rx2_frequency.unwrap_or(0).to_le_bytes());
        p[59] = self.tx_power.unwrap_or(0xFF);
        p[60] = self.fcnt_down.is_some() as u8;
        p[61] = self.adr_enabled as u8;
        encode_blob(NvmRegion::Session, &p, out)
    }

    pub fn decode(blob: &[u8]) -> Result<Self, DecodeError> {
        let p = decode_blob(NvmRegion::Session, blob)?;
        if p.len() != SESSION_PAYLOAD_LEN {
            return Err(DecodeError::BadLength);
        }
        let mut nwkskey = [0u8; 16];
        nwkskey.copy_from_slice(&p[0..16]);
        let mut appskey = [0u8; 16];
        appskey.copy_from_slice(&p[16..32]);
        let rx2_frequency = u32::from_le_bytes([p[55], p[56], p[57], p[58]]);
        Ok(Self {
            nwkskey: NwkSKey::from(nwkskey),
            appskey: AppSKey::from(appskey),
            devaddr: DevAddr::from_wire_bytes([p[32], p[33], p[34], p[35]]),
            fcnt_up_checkpoint: u32::from_le_bytes([p[36], p[37], p[38], p[39]]),
            fcnt_down: (p[60] != 0).then(|| u32::from_le_bytes([p[40], p[41], p[42], p[43]])),
            join_epoch: u32::from_le_bytes([p[44], p[45], p[46], p[47]]),
            data_rate: DR::from(p[48]),
            rx1_delay: u32::from_le_bytes([p[49], p[50], p[51], p[52]]),
            rx1_dr_offset: p[53],
            rx2_data_rate: (p[54] != 0xFF).then(|| DR::from(p[54])),
            rx2_frequency: (rx2_frequency != 0).then_some(rx2_frequency),
            tx_power: (p[59] != 0xFF).then_some(p[59]),
            adr_enabled: p[61] != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> PersistentIdentity {
        PersistentIdentity { dev_nonce: 0x1234, last_join_nonce: Some(77), join_epoch: 3 }
    }

    fn session() -> PersistentSession {
        PersistentSession {
            nwkskey: NwkSKey::from([1; 16]),
            appskey: AppSKey::from([2; 16]),
            devaddr: DevAddr::from_wire_bytes([1, 2, 3, 4]),
            fcnt_up_checkpoint: 320,
            fcnt_down: Some(17),
            join_epoch: 3,
            data_rate: DR::_2,
            rx1_delay: 5000,
            rx1_dr_offset: 1,
            rx2_data_rate: Some(DR::_8),
            rx2_frequency: Some(923_300_000),
            tx_power: None,
            adr_enabled: false,
        }
    }

    #[test]
    fn identity_roundtrip() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = identity().encode(&mut buf);
        assert_eq!(len, HEADER_LEN + IDENTITY_PAYLOAD_LEN);
        assert_eq!(PersistentIdentity::decode(&buf[..len]).unwrap(), identity());
    }

    #[test]
    fn identity_roundtrip_no_join_nonce() {
        let id = PersistentIdentity { dev_nonce: 0, last_join_nonce: None, join_epoch: 0 };
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = id.encode(&mut buf);
        assert_eq!(PersistentIdentity::decode(&buf[..len]).unwrap(), id);
    }

    #[test]
    fn session_roundtrip() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = session().encode(&mut buf);
        assert_eq!(len, HEADER_LEN + SESSION_PAYLOAD_LEN);
        assert_eq!(PersistentSession::decode(&buf[..len]).unwrap(), session());
    }

    #[test]
    fn corrupt_payload_rejected() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = identity().encode(&mut buf);
        buf[len - 1] ^= 0x01;
        assert_eq!(PersistentIdentity::decode(&buf[..len]), Err(DecodeError::BadCrc));
    }

    #[test]
    fn corrupt_header_rejected() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = identity().encode(&mut buf);
        buf[6] ^= 0x01; // length field
        assert_eq!(PersistentIdentity::decode(&buf[..len]), Err(DecodeError::BadLength));
    }

    #[test]
    fn truncated_rejected() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = identity().encode(&mut buf);
        assert_eq!(PersistentIdentity::decode(&buf[..len - 3]), Err(DecodeError::BadLength));
        assert_eq!(PersistentIdentity::decode(&buf[..3]), Err(DecodeError::BadMagic));
        assert_eq!(PersistentIdentity::decode(&[]), Err(DecodeError::BadMagic));
    }

    #[test]
    fn wrong_magic_rejected() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = identity().encode(&mut buf);
        buf[0] = b'X';
        assert_eq!(PersistentIdentity::decode(&buf[..len]), Err(DecodeError::BadMagic));
    }

    #[test]
    fn swapped_region_rejected() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = session().encode(&mut buf);
        assert_eq!(PersistentIdentity::decode(&buf[..len]), Err(DecodeError::RegionMismatch));
    }

    #[test]
    fn schema_version_mismatch_rejected() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = identity().encode(&mut buf);
        buf[5] += 1;
        assert_eq!(PersistentIdentity::decode(&buf[..len]), Err(DecodeError::SchemaVersion));
    }

    #[test]
    fn crc_covers_header_fields() {
        // Flipping region tag alone must not slip past as the other region's valid blob.
        let mut buf = [0u8; MAX_BLOB_LEN];
        let id = PersistentIdentity { dev_nonce: 1, last_join_nonce: None, join_epoch: 0 };
        let len = id.encode(&mut buf);
        buf[4] = region_tag(NvmRegion::Session);
        assert!(PersistentSession::decode(&buf[..len]).is_err());
    }
}
