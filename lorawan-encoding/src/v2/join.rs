//! Zero-copy views of JoinRequest and JoinAccept frames.

use crate::keys::{AppKey, AppSKey, CryptoFactory, Encrypter, NwkSKey, AES128, MIC};
use crate::packet_length::phy::join::{
    JOIN_ACCEPT_LEN, JOIN_ACCEPT_WITH_CFLIST_LEN, JOIN_REQUEST_LEN,
};
use crate::packet_length::phy::MIC_LEN;
use crate::securityhelpers;
use crate::types::{ChannelMask, DLSettings};

use super::types::{arr, DevAddr, DevEui, DevNonce, Frequency, JoinEui, JoinNonce, NetId};
use super::Error;

#[inline]
fn check_mhdr(bytes: &[u8], mtype: u8) -> Result<(), Error> {
    let [mhdr, ..] = bytes else {
        return Err(Error::TooShort);
    };
    if mhdr & 0b11 != 0 {
        return Err(Error::UnsupportedMajorVersion);
    }
    if mhdr >> 5 != mtype {
        return Err(Error::UnexpectedMessageType);
    }
    Ok(())
}

#[inline]
fn extract_mic(bytes: &[u8]) -> MIC {
    MIC(arr(&bytes[bytes.len() - MIC_LEN..]))
}

/// Zero-copy view of a JoinRequest frame.
///
/// The fixed 23-byte length is checked once at parse time; the fixed-size
/// reference makes every accessor's bounds visible to the compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinRequestPayload<'a> {
    bytes: &'a [u8; JOIN_REQUEST_LEN],
}

impl<'a> JoinRequestPayload<'a> {
    /// Parses and structurally validates a JoinRequest.
    #[inline]
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        check_mhdr(bytes, 0)?;
        let bytes = bytes.try_into().map_err(|_| Error::InvalidLength)?;
        Ok(Self { bytes })
    }

    /// The JoinEUI (called AppEUI before LoRaWAN 1.0.4).
    #[inline]
    pub fn join_eui(&self) -> JoinEui {
        JoinEui::from_wire_bytes(arr(&self.bytes[1..9]))
    }

    #[inline]
    pub fn dev_eui(&self) -> DevEui {
        DevEui::from_wire_bytes(arr(&self.bytes[9..17]))
    }

    #[inline]
    pub fn dev_nonce(&self) -> DevNonce {
        DevNonce::from_wire_bytes(arr(&self.bytes[17..19]))
    }

    #[inline]
    pub fn mic(&self) -> MIC {
        extract_mic(self.bytes)
    }

    /// Whether the MIC matches under the device's root key.
    #[inline]
    pub fn validate_mic<C: CryptoFactory>(&self, key: &AppKey, crypto: &C) -> bool {
        let without_mic = &self.bytes[..JOIN_REQUEST_LEN - MIC_LEN];
        self.mic() == securityhelpers::calculate_mic(without_mic, crypto.new_mac(key.inner()))
    }

    /// The raw frame bytes.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

#[inline]
fn validate_join_accept_structure(bytes: &[u8]) -> Result<(), Error> {
    check_mhdr(bytes, 1)?;
    if bytes.len() != JOIN_ACCEPT_LEN && bytes.len() != JOIN_ACCEPT_WITH_CFLIST_LEN {
        return Err(Error::InvalidLength);
    }
    Ok(())
}

/// Zero-copy view of a still-encrypted JoinAccept frame.
///
/// Nothing but the MHDR is readable before decryption; the payload (including
/// its MIC) is encrypted with the AppKey. Hand the mutable buffer to
/// [`DecryptedJoinAcceptPayload::decrypt_in_place`] to proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncryptedJoinAcceptPayload<'a> {
    bytes: &'a [u8],
}

impl<'a> EncryptedJoinAcceptPayload<'a> {
    /// Parses and structurally validates an encrypted JoinAccept.
    #[inline]
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        validate_join_accept_structure(bytes)?;
        Ok(Self { bytes })
    }

    /// The raw frame bytes.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// A decrypted JoinAccept, borrowing the buffer that was decrypted in place.
#[derive(Debug)]
pub struct DecryptedJoinAcceptPayload<'a> {
    bytes: &'a [u8],
}

/// The optional channel frequency list of a JoinAccept, fully owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfList {
    /// CFList type 0: five additional channel frequencies.
    DynamicChannel([Frequency; 5]),
    /// CFList type 1: a 72-entry channel mask for fixed-plan regions.
    FixedChannel(ChannelMask<9>),
}

impl<'a> DecryptedJoinAcceptPayload<'a> {
    /// Parses `buf` and decrypts it in place.
    ///
    /// Does NOT verify the MIC (it only becomes readable after decryption);
    /// prefer [`Self::check_mic_and_decrypt_in_place`].
    #[inline]
    pub fn decrypt_in_place<C: CryptoFactory>(
        buf: &'a mut [u8],
        key: &AppKey,
        crypto: &C,
    ) -> Result<Self, Error> {
        validate_join_accept_structure(buf)?;
        // The device side runs AES *encryption* to invert the server's
        // decrypt-mode transformation, per the spec.
        let aes = crypto.new_enc(key.inner());
        for block in buf[1..].chunks_exact_mut(16) {
            aes.encrypt_block(block);
        }
        Ok(Self { bytes: buf })
    }

    /// Decrypts in place, then verifies the MIC.
    #[inline]
    pub fn check_mic_and_decrypt_in_place<C: CryptoFactory>(
        buf: &'a mut [u8],
        key: &AppKey,
        crypto: &C,
    ) -> Result<Self, Error> {
        let decrypted = Self::decrypt_in_place(buf, key, crypto)?;
        if !decrypted.validate_mic(key, crypto) {
            return Err(Error::InvalidMic);
        }
        Ok(decrypted)
    }

    /// Whether the MIC matches under the device's root key.
    #[inline]
    pub fn validate_mic<C: CryptoFactory>(&self, key: &AppKey, crypto: &C) -> bool {
        let without_mic = &self.bytes[..self.bytes.len() - MIC_LEN];
        self.mic() == securityhelpers::calculate_mic(without_mic, crypto.new_mac(key.inner()))
    }

    /// The server nonce (called AppNonce before LoRaWAN 1.0.4).
    #[inline]
    pub fn join_nonce(&self) -> JoinNonce {
        JoinNonce::from_wire_bytes(arr(&self.bytes[1..4]))
    }

    #[inline]
    pub fn net_id(&self) -> NetId {
        NetId::from_wire_bytes(arr(&self.bytes[4..7]))
    }

    #[inline]
    pub fn dev_addr(&self) -> DevAddr {
        DevAddr::from_wire_bytes(arr(&self.bytes[7..11]))
    }

    #[inline]
    pub fn dl_settings(&self) -> DLSettings {
        DLSettings::new(self.bytes[11])
    }

    /// The RX1 delay in seconds (1..=15; the wire value 0 means 1).
    #[inline]
    pub fn rx_delay(&self) -> u8 {
        self.bytes[12] & 0x0f
    }

    /// The channel frequency list, when present and of a known type.
    ///
    /// Returns `None` both when the frame carries no CFList and when the
    /// CFList type octet is RFU, matching the old parser.
    #[inline]
    pub fn c_f_list(&self) -> Option<CfList> {
        if self.bytes.len() == JOIN_ACCEPT_LEN {
            return None;
        }
        let cflist = &self.bytes[13..29];
        match cflist[15] {
            0 => {
                let mut freqs = [Frequency::default(); 5];
                for (freq, chunk) in freqs.iter_mut().zip(cflist.chunks_exact(3)) {
                    *freq = Frequency::from_wire_bytes(arr(chunk));
                }
                Some(CfList::DynamicChannel(freqs))
            }
            1 => Some(CfList::FixedChannel(ChannelMask::new_from_raw(&cflist[..9]))),
            _ => None,
        }
    }

    #[inline]
    pub fn mic(&self) -> MIC {
        extract_mic(self.bytes)
    }

    /// The raw (decrypted) frame bytes.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Derives the network session key for this join.
    #[inline]
    pub fn derive_nwkskey<C: CryptoFactory>(
        &self,
        dev_nonce: DevNonce,
        key: &AppKey,
        crypto: &C,
    ) -> NwkSKey {
        NwkSKey(self.derive_session_key(0x01, dev_nonce, key, crypto))
    }

    /// Derives the application session key for this join.
    #[inline]
    pub fn derive_appskey<C: CryptoFactory>(
        &self,
        dev_nonce: DevNonce,
        key: &AppKey,
        crypto: &C,
    ) -> AppSKey {
        AppSKey(self.derive_session_key(0x02, dev_nonce, key, crypto))
    }

    fn derive_session_key<C: CryptoFactory>(
        &self,
        first_byte: u8,
        dev_nonce: DevNonce,
        key: &AppKey,
        crypto: &C,
    ) -> AES128 {
        let cipher = crypto.new_enc(key.inner());
        let mut block = [0u8; 16];
        block[0] = first_byte;
        block[1..4].copy_from_slice(self.join_nonce().as_wire_bytes());
        block[4..7].copy_from_slice(self.net_id().as_wire_bytes());
        block[7..9].copy_from_slice(dev_nonce.as_wire_bytes());
        cipher.encrypt_block(&mut block);
        AES128(block)
    }
}
