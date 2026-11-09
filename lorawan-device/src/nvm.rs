//! Non-volatile storage for the LoRaWAN 1.0.4 join rules.
//!
//! Providing a store to the device enables the 1.0.4 behavior: DevNonce is a persisted
//! monotonic counter and a JoinAccept repeating the last JoinNonce is rejected as a replay.
//! With the default [`NoNvm`] the stack behaves as LoRaWAN 1.0.2 (random DevNonce, no
//! persistence).
//!
//! The version is selected at compile time by the store type, via
//! [`NonVolatileStore::PERSISTENT`], so a build cannot claim 1.0.4 without somewhere to
//! keep its anti-replay state.
//!
//! One region is stored, [`NvmRegion::Identity`]: the join counters and the certification
//! port state. It is written around joins and when the port is closed, and never reset for
//! the life of the device. The session itself is not persisted: after a reset an OTAA
//! device joins again and the JoinAccept supplies the network's parameters.
//!
//! Surviving power loss mid-write is the store implementation's responsibility (two-slot
//! ping-pong or a crate such as `sequential-storage`). Blobs carry a CRC-protected header,
//! so a torn write is detected and treated as an absent region. For the identity region
//! that fallback restarts the DevNonce at 0, which a join server rejects until the counter
//! passes its last recorded value, so identity writes must be atomic.

use core::fmt::Debug;

use lorawan::parser::JoinNonce;

/// Which logical slot is being read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[non_exhaustive]
pub enum NvmRegion {
    /// Join anti-replay counters and the certification port state.
    Identity,
}

/// Largest encoded blob (header included) the stack will ever pass to `save` or expect
/// from `load`. Store implementations may size internal buffers with this.
pub const MAX_BLOB_LEN: usize = 32;

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

/// Durable per-device state. Written around joins and when the certification port is
/// closed, never reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct PersistentIdentity {
    /// 1.0.4 monotonic join counter; +1 per JoinRequest, never resets. `u16::MAX` marks
    /// exhaustion: no further join is attempted with this identity.
    pub dev_nonce: u16,
    /// JoinNonce of the last accepted JoinAccept, kept to reject Join-accept replays as
    /// recommended by TR001 (Remedy #1). An accept carrying the same value is rejected.
    /// TR001 goes further and accepts only a strictly greater JoinNonce, but that relies on
    /// a Join Server that counts JoinNonce (TR001's `AppNonceCounter`); 1.0.4 only promises
    /// a non-repeating value, so the stricter check would reject servers that pick random
    /// nonces. `None` until the first successful join.
    pub last_join_nonce: Option<JoinNonce>,
    /// FPort 224 closed for good, by the certification protocol's DutFPort224DisableReq
    /// or by the application. Only acted on with the `certification` feature; stored
    /// regardless so the blob layout does not depend on features.
    pub fport224_disabled: bool,
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
    /// Blob belongs to another region (slots swapped by the store).
    RegionMismatch,
    /// Length field disagrees with the buffer or the expected payload size.
    BadLength,
    /// Payload failed its integrity check (torn or corrupted write).
    BadCrc,
    /// A field holds a value it can never take.
    BadValue,
}

const MAGIC: [u8; 4] = *b"LWNV";
const HEADER_LEN: usize = 12;
const IDENTITY_SCHEMA_VERSION: u8 = 1;
const IDENTITY_PAYLOAD_LEN: usize = 7;
/// Sentinel encoding of `last_join_nonce: None`; JoinNonce is a 3-byte field on the
/// wire, so this value can never be a real nonce.
const JOIN_NONCE_NONE: u32 = 0xFFFF_FFFF;
const JOIN_NONCE_MAX: u32 = 0x00FF_FFFF;

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
    }
}

fn schema_version(region: NvmRegion) -> u8 {
    match region {
        NvmRegion::Identity => IDENTITY_SCHEMA_VERSION,
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
        let join_nonce = self.last_join_nonce.map_or(JOIN_NONCE_NONE, |n| n.value());
        p[2..6].copy_from_slice(&join_nonce.to_le_bytes());
        p[6] = self.fport224_disabled as u8;
        encode_blob(NvmRegion::Identity, &p, out)
    }

    pub fn decode(blob: &[u8]) -> Result<Self, DecodeError> {
        let p = decode_blob(NvmRegion::Identity, blob)?;
        if p.len() != IDENTITY_PAYLOAD_LEN {
            return Err(DecodeError::BadLength);
        }
        let last_join_nonce = match u32::from_le_bytes([p[2], p[3], p[4], p[5]]) {
            JOIN_NONCE_NONE => None,
            n if n <= JOIN_NONCE_MAX => Some(JoinNonce::from_value(n)),
            _ => return Err(DecodeError::BadValue),
        };
        Ok(Self {
            dev_nonce: u16::from_le_bytes([p[0], p[1]]),
            last_join_nonce,
            fport224_disabled: p[6] != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> PersistentIdentity {
        PersistentIdentity {
            dev_nonce: 0x1234,
            last_join_nonce: Some(JoinNonce::from_value(0x00AB_CDEF)),
            fport224_disabled: true,
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
    fn identity_roundtrip_defaults() {
        let id = PersistentIdentity::default();
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = id.encode(&mut buf);
        assert_eq!(PersistentIdentity::decode(&buf[..len]).unwrap(), id);
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
    fn join_nonce_wider_than_three_octets_rejected() {
        let mut payload = [0u8; IDENTITY_PAYLOAD_LEN];
        payload[2..6].copy_from_slice(&(JOIN_NONCE_MAX + 1).to_le_bytes());
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = encode_blob(NvmRegion::Identity, &payload, &mut buf);
        assert_eq!(PersistentIdentity::decode(&buf[..len]), Err(DecodeError::BadValue));
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
    fn other_region_rejected() {
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = identity().encode(&mut buf);
        buf[4] += 1;
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
    fn wrong_payload_length_rejected() {
        // A consistent header around a payload of the wrong size for the schema.
        let mut buf = [0u8; MAX_BLOB_LEN];
        let len = encode_blob(NvmRegion::Identity, &[0u8; IDENTITY_PAYLOAD_LEN + 1], &mut buf);
        assert_eq!(PersistentIdentity::decode(&buf[..len]), Err(DecodeError::BadLength));
    }
}
