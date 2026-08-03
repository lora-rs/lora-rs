//! Zero-copy views of data frames with in-place decryption.

use crate::keys::{AppSKey, CryptoFactory, NwkSKey, AES128, MIC};
use crate::packet_length::phy::{MHDR_LEN, MIC_LEN};
use crate::securityhelpers;

use super::types::{arr, DevAddr, FCtrl};
use super::Error;

/// The four data frame message types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum DataFrameType {
    UnconfirmedUp,
    UnconfirmedDown,
    ConfirmedUp,
    ConfirmedDown,
}

impl DataFrameType {
    #[inline]
    fn from_mhdr(mhdr: u8) -> Option<Self> {
        match mhdr >> 5 {
            2 => Some(Self::UnconfirmedUp),
            3 => Some(Self::UnconfirmedDown),
            4 => Some(Self::ConfirmedUp),
            5 => Some(Self::ConfirmedDown),
            _ => None,
        }
    }

    #[inline]
    pub fn is_uplink(self) -> bool {
        matches!(self, Self::UnconfirmedUp | Self::ConfirmedUp)
    }

    #[inline]
    pub fn is_confirmed(self) -> bool {
        matches!(self, Self::ConfirmedUp | Self::ConfirmedDown)
    }
}

/// Zero-copy view of the FHDR of a data frame.
///
/// Only constructed by [`EncryptedDataPayload::parse`] /
/// [`DecryptedDataPayload`], which validate the length invariant
/// (`bytes.len() == 7 + f_opts_len`), so accessors cannot panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fhdr<'a> {
    bytes: &'a [u8],
    uplink: bool,
}

impl<'a> Fhdr<'a> {
    #[inline]
    pub fn dev_addr(&self) -> DevAddr {
        DevAddr::from_wire_bytes(arr(&self.bytes[0..4]))
    }

    #[inline]
    pub fn fctrl(&self) -> FCtrl {
        FCtrl::new(self.bytes[4], self.uplink)
    }

    /// The truncated 16-bit frame counter from the wire.
    #[inline]
    pub fn fcnt(&self) -> u16 {
        u16::from_le_bytes([self.bytes[5], self.bytes[6]])
    }

    /// The FOpts bytes (may be empty). Length was validated at parse time.
    #[inline]
    pub fn f_opts(&self) -> &'a [u8] {
        &self.bytes[7..]
    }
}

/// Layout facts computed once during validation and shared by both payload
/// views. Offsets are guaranteed in bounds for the buffer they were computed
/// from.
#[derive(Debug, Clone, Copy)]
struct Layout {
    frame_type: DataFrameType,
    fhdr_len: usize,
    /// Offset of the FPort byte, if present.
    f_port_offset: Option<usize>,
    /// FRMPayload byte range (empty if absent).
    frm_start: usize,
    frm_end: usize,
}

impl Layout {
    /// Validates the structure of a data frame. This is the single place
    /// bounds are checked; every view accessor relies on it.
    #[inline]
    fn validate(bytes: &[u8]) -> Result<Self, Error> {
        // MHDR (1) + DevAddr (4) + FCtrl (1) + FCnt (2) + MIC (4)
        const MIN_LEN: usize = MHDR_LEN + 7 + MIC_LEN;
        if bytes.len() < MIN_LEN {
            return Err(Error::TooShort);
        }
        let mhdr = bytes[0];
        if mhdr & 0b11 != 0 {
            return Err(Error::UnsupportedMajorVersion);
        }
        let frame_type = DataFrameType::from_mhdr(mhdr).ok_or(Error::NotADataFrame)?;
        let fhdr_len = 7 + (bytes[5] & 0x0f) as usize;
        let mic_offset = bytes.len() - MIC_LEN;
        // FHDR must fit between MHDR and MIC.
        if MHDR_LEN + fhdr_len > mic_offset {
            return Err(Error::TruncatedFhdr);
        }
        let after_fhdr = MHDR_LEN + fhdr_len;
        let (f_port_offset, frm_start) = if after_fhdr < mic_offset {
            (Some(after_fhdr), after_fhdr + 1)
        } else {
            (None, after_fhdr)
        };
        Ok(Self { frame_type, fhdr_len, f_port_offset, frm_start, frm_end: mic_offset })
    }
}

macro_rules! data_view_accessors {
    () => {
        #[inline]
        pub fn frame_type(&self) -> DataFrameType {
            self.layout.frame_type
        }

        #[inline]
        pub fn is_uplink(&self) -> bool {
            self.layout.frame_type.is_uplink()
        }

        #[inline]
        pub fn is_confirmed(&self) -> bool {
            self.layout.frame_type.is_confirmed()
        }

        #[inline]
        pub fn fhdr(&self) -> Fhdr<'a> {
            Fhdr {
                bytes: &self.bytes[MHDR_LEN..MHDR_LEN + self.layout.fhdr_len],
                uplink: self.is_uplink(),
            }
        }

        /// The FPort, if the frame carries one.
        ///
        /// Unlike the old parser, a present FPort with an empty FRMPayload is
        /// reported as `Some`.
        #[inline]
        pub fn f_port(&self) -> Option<u8> {
            self.layout.f_port_offset.map(|off| self.bytes[off])
        }

        #[inline]
        pub fn mic(&self) -> MIC {
            let mic = &self.bytes[self.bytes.len() - MIC_LEN..];
            MIC([mic[0], mic[1], mic[2], mic[3]])
        }

        /// The raw frame bytes.
        #[inline]
        pub fn as_bytes(&self) -> &'a [u8] {
            self.bytes
        }
    };
}

/// Zero-copy read-only view of an encrypted data frame.
///
/// One monomorphization regardless of where the bytes live; inspect the
/// header, look up session keys by [`DevAddr`], check the MIC, then hand the
/// mutable buffer to [`DecryptedDataPayload::decrypt_in_place`].
#[derive(Debug, Clone, Copy)]
pub struct EncryptedDataPayload<'a> {
    bytes: &'a [u8],
    layout: Layout,
}

impl<'a> EncryptedDataPayload<'a> {
    /// Parses and structurally validates an encrypted data frame.
    #[inline]
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        let layout = Layout::validate(bytes)?;
        Ok(Self { bytes, layout })
    }

    data_view_accessors!();

    /// Whether the MIC matches under `key` and the given 32-bit frame counter.
    #[inline]
    pub fn validate_mic<C: CryptoFactory>(&self, key: &AES128, fcnt: u32, crypto: &C) -> bool {
        let without_mic = &self.bytes[..self.bytes.len() - MIC_LEN];
        self.mic() == securityhelpers::calculate_data_mic(without_mic, crypto.new_mac(key), fcnt)
    }
}

/// A decrypted data frame, borrowing the buffer that was decrypted in place.
///
/// Only obtainable through [`Self::decrypt_in_place`] or
/// [`Self::check_mic_and_decrypt_in_place`], which preserves the
/// encrypted/decrypted typestate of the old design without a storage generic.
#[derive(Debug)]
pub struct DecryptedDataPayload<'a> {
    bytes: &'a [u8],
    layout: Layout,
}

/// The decrypted FRMPayload contents.
#[derive(Debug, PartialEq, Eq)]
pub enum FrmPayload<'a> {
    /// Application data (FPort > 0).
    Data(&'a [u8]),
    /// MAC commands (FPort == 0).
    MacCommands(&'a [u8]),
    /// The frame carries no FRMPayload.
    None,
}

impl<'a> DecryptedDataPayload<'a> {
    /// Parses `buf` and decrypts its FRMPayload in place.
    ///
    /// Does NOT verify the MIC; prefer [`Self::check_mic_and_decrypt_in_place`]
    /// unless the MIC was already checked via
    /// [`EncryptedDataPayload::validate_mic`].
    ///
    /// Key selection follows the spec: `app_skey` decrypts FPort > 0,
    /// `nwk_skey` decrypts FPort 0 (MAC commands). Only the key the frame
    /// actually needs must be present. `fcnt` supplies the upper 16 bits of
    /// the frame counter; the lower 16 come from the wire.
    #[inline]
    pub fn decrypt_in_place<C: CryptoFactory>(
        buf: &'a mut [u8],
        nwk_skey: Option<&NwkSKey>,
        app_skey: Option<&AppSKey>,
        fcnt: u32,
        crypto: &C,
    ) -> Result<Self, Error> {
        let layout = Layout::validate(buf)?;
        let uses_app_key = matches!(layout.f_port_offset, Some(off) if buf[off] != 0);
        let key = if uses_app_key {
            app_skey.map(|k| k.inner())
        } else {
            nwk_skey.map(|k| k.inner())
        };
        let key = key.ok_or(Error::MissingKey)?;
        if layout.frm_start < layout.frm_end {
            let wire_fcnt = u16::from_le_bytes([buf[6], buf[7]]);
            let full_fcnt = ((fcnt >> 16) << 16) | u32::from(wire_fcnt);
            securityhelpers::encrypt_frm_data_payload(
                buf,
                layout.frm_start,
                layout.frm_end,
                full_fcnt,
                &crypto.new_enc(key),
            );
        }
        Ok(Self { bytes: buf, layout })
    }

    /// Verifies the MIC with `nwk_skey`, then decrypts in place.
    #[inline]
    pub fn check_mic_and_decrypt_in_place<C: CryptoFactory>(
        buf: &'a mut [u8],
        nwk_skey: &NwkSKey,
        app_skey: Option<&AppSKey>,
        fcnt: u32,
        crypto: &C,
    ) -> Result<Self, Error> {
        if !EncryptedDataPayload::parse(buf)?.validate_mic(nwk_skey.inner(), fcnt, crypto) {
            return Err(Error::InvalidMic);
        }
        Self::decrypt_in_place(buf, Some(nwk_skey), app_skey, fcnt, crypto)
    }

    data_view_accessors!();

    /// The decrypted FRMPayload.
    #[inline]
    pub fn frm_payload(&self) -> FrmPayload<'a> {
        let frm = &self.bytes[self.layout.frm_start..self.layout.frm_end];
        match self.f_port() {
            None => FrmPayload::None,
            Some(0) => FrmPayload::MacCommands(frm),
            Some(_) => FrmPayload::Data(frm),
        }
    }
}
