//! Building frames into caller-provided buffers.
//!
//! Instead of the old setter-chain creators (where some setters return
//! `&mut Self` and others `Result<&mut Self>`, breaking fluent chains), a
//! frame is described by a plain struct literal: every field is visible and
//! required at the construction site, and the validation that cannot be
//! expressed in the types happens once in `build_into`.
//!
//! The types rule out most invalid frames statically:
//!
//! * [`Payload::Data`] carries a [`NonZeroU8`] FPort, so application data on
//!   port 0 is unrepresentable.
//! * [`Payload`] is an enum, so MAC commands and application data in the same
//!   FRMPayload are unrepresentable.
//! * Wire fields are the owned types from [`crate::parser`], so byte order
//!   mistakes are confined to their constructors.

use core::num::NonZeroU8;

use crate::keys::{AppKey, AppSKey, CryptoFactory, Decrypter, NwkSKey};
use crate::packet_length::phy::join::{
    JOIN_ACCEPT_LEN, JOIN_ACCEPT_WITH_CFLIST_LEN, JOIN_REQUEST_LEN,
};
use crate::packet_length::phy::{MHDR_LEN, MIC_LEN};
use crate::securityhelpers;
use crate::types::DLSettings;

use crate::parser::{
    CfList, DataFrameType, DevAddr, DevEui, DevNonce, Error, JoinEui, JoinNonce, NetId,
};

fn write_mic<C: CryptoFactory>(out: &mut [u8], key: &crate::keys::AES128, crypto: &C) {
    let mic_offset = out.len() - MIC_LEN;
    let mic = securityhelpers::calculate_mic(&out[..mic_offset], crypto.new_mac(key));
    out[mic_offset..].copy_from_slice(&mic.0);
}

/// A JoinRequest, ready to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinRequest {
    /// Called AppEUI before LoRaWAN 1.0.4.
    pub join_eui: JoinEui,
    pub dev_eui: DevEui,
    pub dev_nonce: DevNonce,
}

impl JoinRequest {
    /// Writes the frame into the front of `buf` with the MIC set, returning
    /// the built bytes.
    pub fn build_into<'a, C: CryptoFactory>(
        &self,
        buf: &'a mut [u8],
        key: &AppKey,
        crypto: &C,
    ) -> Result<&'a [u8], Error> {
        let out = buf.get_mut(..JOIN_REQUEST_LEN).ok_or(Error::BufferTooShort)?;
        out[0] = 0x00;
        out[1..9].copy_from_slice(self.join_eui.as_wire_bytes());
        out[9..17].copy_from_slice(self.dev_eui.as_wire_bytes());
        out[17..19].copy_from_slice(self.dev_nonce.as_wire_bytes());
        write_mic(out, key.inner(), crypto);
        Ok(out)
    }
}

/// A JoinAccept, ready to build.
///
/// Reuses the owned [`CfList`] that parsing produces, so a network server can
/// parse, tweak, and rebuild without conversions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinAccept {
    /// Called AppNonce before LoRaWAN 1.0.4.
    pub join_nonce: JoinNonce,
    pub net_id: NetId,
    pub dev_addr: DevAddr,
    pub dl_settings: DLSettings,
    /// RX1 delay in seconds, 0..=15 (0 is interpreted as 1 by devices).
    pub rx_delay: u8,
    pub c_f_list: Option<CfList>,
}

impl JoinAccept {
    /// Writes the encrypted frame into the front of `buf` with the MIC set,
    /// returning the built bytes.
    pub fn build_into<'a, C: CryptoFactory>(
        &self,
        buf: &'a mut [u8],
        key: &AppKey,
        crypto: &C,
    ) -> Result<&'a [u8], Error> {
        let len = if self.c_f_list.is_some() {
            JOIN_ACCEPT_WITH_CFLIST_LEN
        } else {
            JOIN_ACCEPT_LEN
        };
        let out = buf.get_mut(..len).ok_or(Error::BufferTooShort)?;
        out[0] = 0x20;
        out[1..4].copy_from_slice(self.join_nonce.as_wire_bytes());
        out[4..7].copy_from_slice(self.net_id.as_wire_bytes());
        out[7..11].copy_from_slice(self.dev_addr.as_wire_bytes());
        out[11] = self.dl_settings.raw_value();
        out[12] = self.rx_delay & 0x0f;
        match &self.c_f_list {
            None => {}
            Some(CfList::DynamicChannel(freqs)) => {
                for (i, freq) in freqs.iter().enumerate() {
                    out[13 + 3 * i..16 + 3 * i].copy_from_slice(freq.as_wire_bytes());
                }
                out[28] = 0; // CFList type
            }
            Some(CfList::FixedChannel(mask)) => {
                out[13..22].copy_from_slice(mask.as_ref());
                out[22..28].fill(0);
                out[28] = 1; // CFList type
            }
        }
        write_mic(out, key.inner(), crypto);
        // The server encrypts by running AES in *decrypt* mode so that the
        // device can decrypt with the cheaper encrypt mode. MHDR stays clear;
        // the MIC is inside the encrypted region.
        let aes = crypto.new_dec(key.inner());
        for block in out[MHDR_LEN..].chunks_exact_mut(16) {
            aes.decrypt_block(block);
        }
        Ok(out)
    }
}

/// The FRMPayload of a data frame to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Payload<'a> {
    /// No FRMPayload and no FPort byte.
    #[default]
    None,
    /// Application data on the given port, encrypted with the AppSKey.
    ///
    /// [`NonZeroU8`] makes application data on port 0 unrepresentable.
    Data { f_port: NonZeroU8, data: &'a [u8] },
    /// MAC commands on port 0, encrypted with the NwkSKey.
    MacCommands(&'a [u8]),
}

/// A data frame, ready to build.
///
/// Construct with a struct literal, using `..Default::default()` for the
/// fields you don't care about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataFrame<'a> {
    pub frame_type: DataFrameType,
    pub dev_addr: DevAddr,
    /// ADR enabled (FCtrl bit 7).
    pub adr: bool,
    /// ADR ACK requested (FCtrl bit 6); written on uplinks only.
    pub adr_ack_req: bool,
    /// Acknowledgment of the previous confirmed frame (FCtrl bit 5).
    pub ack: bool,
    /// More downlink pending (FCtrl bit 4); written on downlinks only.
    pub f_pending: bool,
    /// The full 32-bit frame counter. The low 16 bits go on the wire; the
    /// rest participates in encryption and the MIC.
    pub fcnt: u32,
    /// Piggybacked MAC commands, at most 15 bytes, sent in clear.
    pub f_opts: &'a [u8],
    pub payload: Payload<'a>,
}

impl Default for DataFrame<'_> {
    fn default() -> Self {
        Self {
            frame_type: DataFrameType::UnconfirmedUp,
            dev_addr: DevAddr::from_value(0),
            adr: false,
            adr_ack_req: false,
            ack: false,
            f_pending: false,
            fcnt: 0,
            f_opts: &[],
            payload: Payload::None,
        }
    }
}

impl DataFrame<'_> {
    fn mhdr(&self) -> u8 {
        match self.frame_type {
            DataFrameType::UnconfirmedUp => 0x40,
            DataFrameType::UnconfirmedDown => 0x60,
            DataFrameType::ConfirmedUp => 0x80,
            DataFrameType::ConfirmedDown => 0xa0,
        }
    }

    fn fctrl(&self) -> u8 {
        let mut b = self.f_opts.len() as u8;
        if self.adr {
            b |= 0x80;
        }
        if self.adr_ack_req && self.frame_type.is_uplink() {
            b |= 0x40;
        }
        if self.ack {
            b |= 0x20;
        }
        if self.f_pending && !self.frame_type.is_uplink() {
            b |= 0x10;
        }
        b
    }

    /// Writes the encrypted frame into the front of `buf` with the MIC set,
    /// returning the built bytes.
    ///
    /// `nwk_skey` is always required (MIC, and FPort-0 encryption);
    /// `app_skey` only when the payload is [`Payload::Data`].
    pub fn build_into<'a, C: CryptoFactory>(
        &self,
        buf: &'a mut [u8],
        nwk_skey: &NwkSKey,
        app_skey: Option<&AppSKey>,
        crypto: &C,
    ) -> Result<&'a [u8], Error> {
        if self.f_opts.len() > 15 {
            return Err(Error::FOptsTooLong);
        }
        let (f_port, frm, enc_key) = match self.payload {
            Payload::None => (None, &[][..], nwk_skey.inner()),
            Payload::Data { f_port, data } => {
                let key = app_skey.ok_or(Error::MissingKey)?;
                (Some(f_port.get()), data, key.inner())
            }
            Payload::MacCommands(cmds) => {
                // The spec forbids FPort 0 when FOpts is non-empty: MAC
                // commands go in one place or the other, never both.
                if !self.f_opts.is_empty() {
                    return Err(Error::FOptsWithFPortZero);
                }
                (Some(0), cmds, nwk_skey.inner())
            }
        };

        let fhdr_len = 7 + self.f_opts.len();
        let total = MHDR_LEN + fhdr_len + f_port.map_or(0, |_| 1) + frm.len() + MIC_LEN;
        let out = buf.get_mut(..total).ok_or(Error::BufferTooShort)?;

        out[0] = self.mhdr();
        out[1..5].copy_from_slice(self.dev_addr.as_wire_bytes());
        out[5] = self.fctrl();
        out[6..8].copy_from_slice(&(self.fcnt as u16).to_le_bytes());
        out[8..8 + self.f_opts.len()].copy_from_slice(self.f_opts);
        let mut cursor = MHDR_LEN + fhdr_len;
        if let Some(port) = f_port {
            out[cursor] = port;
            cursor += 1;
        }
        out[cursor..cursor + frm.len()].copy_from_slice(frm);
        if !frm.is_empty() {
            securityhelpers::encrypt_frm_data_payload(
                out,
                cursor,
                cursor + frm.len(),
                self.fcnt,
                &crypto.new_enc(enc_key),
            );
        }
        let mic_offset = total - MIC_LEN;
        let mic = securityhelpers::calculate_data_mic(
            &out[..mic_offset],
            crypto.new_mac(nwk_skey.inner()),
            self.fcnt,
        );
        out[mic_offset..].copy_from_slice(&mic.0);
        Ok(out)
    }
}
