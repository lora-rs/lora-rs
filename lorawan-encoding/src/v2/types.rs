//! Owned newtypes for fixed-size wire fields.
//!
//! Every multi-byte LoRaWAN field is transmitted least-significant-byte
//! first. Each type here stores the bytes in wire order and offers explicit
//! conversions to and from the logical integer value, so byte order is
//! decided at exactly one place instead of at every call site.

/// Error from parsing a fixed-size hex string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseHexError;

impl core::fmt::Display for ParseHexError {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("expected a fixed-size hex string")
    }
}

impl core::error::Error for ParseHexError {}

/// Copies a slice into a fixed array. Callers guarantee `slice.len() == N`
/// through parse-time validation (see `Layout` in the data module and the
/// length checks in the join module).
#[inline]
pub(crate) fn arr<const N: usize>(slice: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    out.copy_from_slice(slice);
    out
}

macro_rules! wire_value_newtype {
    (
        $(#[$outer:meta])*
        $name:ident([u8; $n:literal], $int:ty)
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
        pub struct $name([u8; $n]);

        impl $name {
            pub const BYTE_LEN: usize = $n;

            /// Constructs from bytes in wire (transmission) order, LSB first.
            #[inline]
            pub const fn from_wire_bytes(bytes: [u8; $n]) -> Self {
                Self(bytes)
            }

            /// Constructs from the logical value.
            #[inline]
            pub fn from_value(value: $int) -> Self {
                let le = value.to_le_bytes();
                let mut bytes = [0u8; $n];
                bytes.copy_from_slice(&le[..$n]);
                Self(bytes)
            }

            /// The bytes in wire (transmission) order, LSB first.
            #[inline]
            pub const fn as_wire_bytes(&self) -> &[u8; $n] {
                &self.0
            }

            /// The logical value.
            #[inline]
            pub fn value(&self) -> $int {
                let mut le = [0u8; core::mem::size_of::<$int>()];
                le[..$n].copy_from_slice(&self.0);
                <$int>::from_le_bytes(le)
            }
        }

        impl From<$int> for $name {
            #[inline]
            fn from(value: $int) -> Self {
                Self::from_value(value)
            }
        }

        impl From<$name> for $int {
            #[inline]
            fn from(v: $name) -> Self {
                v.value()
            }
        }

        /// Hex-formats the logical value MSB first, the conventional display
        /// order.
        impl core::fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{:0width$x}", self.value(), width = 2 * $n)
            }
        }

        /// Parses the conventional MSB-first hex form.
        impl core::str::FromStr for $name {
            type Err = ParseHexError;

            #[inline]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if s.len() != 2 * $n {
                    return Err(ParseHexError);
                }
                let value = <$int>::from_str_radix(s, 16).map_err(|_| ParseHexError)?;
                Ok(Self::from_value(value))
            }
        }
    };
}

wire_value_newtype! {
    /// A 32-bit LoRaWAN device address.
    ///
    /// `DevAddr::from_value(0x01020304).as_wire_bytes()` is
    /// `[0x04, 0x03, 0x02, 0x01]`.
    DevAddr([u8; 4], u32)
}

impl DevAddr {
    /// The 7-bit network identifier (NwkID) in the most significant bits.
    #[inline]
    pub const fn nwk_id(&self) -> u8 {
        self.0[3] >> 1
    }
}

wire_value_newtype! {
    /// The 64-bit extended unique identifier of an end-device.
    ///
    /// Matches the conventions of [`crate::keys::DevEui`]: wire bytes are
    /// LSB first, `Display`/`FromStr` use the MSB-first form printed on
    /// device labels.
    DevEui([u8; 8], u64)
}

wire_value_newtype! {
    /// The 64-bit identifier of the join server (AppEUI before LoRaWAN
    /// 1.0.4).
    JoinEui([u8; 8], u64)
}

impl From<crate::keys::DevEui> for DevEui {
    #[inline]
    fn from(eui: crate::keys::DevEui) -> Self {
        Self::from_wire_bytes(arr(eui.as_ref()))
    }
}

impl From<DevEui> for crate::keys::DevEui {
    #[inline]
    fn from(eui: DevEui) -> Self {
        Self::from(*eui.as_wire_bytes())
    }
}

impl From<crate::keys::AppEui> for JoinEui {
    #[inline]
    fn from(eui: crate::keys::AppEui) -> Self {
        Self::from_wire_bytes(arr(eui.as_ref()))
    }
}

impl From<JoinEui> for crate::keys::AppEui {
    #[inline]
    fn from(eui: JoinEui) -> Self {
        Self::from(*eui.as_wire_bytes())
    }
}

wire_value_newtype! {
    /// The 16-bit device nonce from a JoinRequest.
    DevNonce([u8; 2], u16)
}

wire_value_newtype! {
    /// The 24-bit server nonce from a JoinAccept.
    ///
    /// Named AppNonce before LoRaWAN 1.0.4.
    JoinNonce([u8; 3], u32)
}

wire_value_newtype! {
    /// The 24-bit network identifier from a JoinAccept.
    NetId([u8; 3], u32)
}

/// A channel frequency, stored as the raw 24-bit wire value (units of 100 Hz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct Frequency([u8; 3]);

impl Frequency {
    /// Constructs from bytes in wire (transmission) order, LSB first.
    #[inline]
    pub const fn from_wire_bytes(bytes: [u8; 3]) -> Self {
        Self(bytes)
    }

    /// Constructs from a frequency in Hz. Sub-100 Hz precision is lost.
    #[inline]
    pub fn from_hz(hz: u32) -> Self {
        let raw = hz / 100;
        let le = raw.to_le_bytes();
        Self([le[0], le[1], le[2]])
    }

    /// The frequency in Hz.
    #[inline]
    pub fn hz(&self) -> u32 {
        u32::from_le_bytes([self.0[0], self.0[1], self.0[2], 0]) * 100
    }

    /// The bytes in wire (transmission) order, LSB first.
    #[inline]
    pub const fn as_wire_bytes(&self) -> &[u8; 3] {
        &self.0
    }
}

/// The FCtrl octet, interpreted for the frame direction it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct FCtrl {
    byte: u8,
    uplink: bool,
}

impl FCtrl {
    #[inline]
    pub const fn new(byte: u8, uplink: bool) -> Self {
        Self { byte, uplink }
    }

    #[inline]
    pub const fn adr(&self) -> bool {
        self.byte & 0x80 != 0
    }

    /// ADR ACK requested; uplink only.
    #[inline]
    pub const fn adr_ack_req(&self) -> bool {
        self.uplink && self.byte & 0x40 != 0
    }

    #[inline]
    pub const fn ack(&self) -> bool {
        self.byte & 0x20 != 0
    }

    /// More downlink pending; downlink only.
    #[inline]
    pub const fn f_pending(&self) -> bool {
        !self.uplink && self.byte & 0x10 != 0
    }

    #[inline]
    pub const fn f_opts_len(&self) -> usize {
        (self.byte & 0x0f) as usize
    }

    #[inline]
    pub const fn raw_value(&self) -> u8 {
        self.byte
    }
}
