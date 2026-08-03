//! Flash-size comparison harness: old generic-over-storage API vs the v2
//! borrowed-view API, on thumbv7em-none-eabi.
//!
//! Each feature selects one configuration; all do the same work (parse a
//! data frame, read header fields, validate MIC, decrypt, fold the payload,
//! plus the join path) through a dummy crypto factory so that AES tables
//! don't drown out the parser code being measured:
//!
//! * `old1`: old API, one storage type (`&mut [u8]`)
//! * `old3`: old API, three storage types (slice, owned array, newtype
//!   wrapper) — the realistic cost when a program mixes storages
//! * `new1`: v2 API, one entry point
//! * `new3`: v2 API, three entry points with the same three storages, all
//!   funneling into the single slice-based implementation
//!
//! Measure with `nm --print-size` summing text symbols from the lorawan
//! crate (see run.sh).

#![no_std]

use lorawan::keys::{AppKey, CryptoFactory, Decrypter, Encrypter, Mac, AES128};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

// --- Dummy crypto: tiny, so the measurement is parser code, not AES ---

struct DumEnc;
impl Encrypter for DumEnc {
    fn encrypt_block(&self, block: &mut [u8]) {
        for b in block {
            *b = b.wrapping_add(0x5a);
        }
    }
}
impl Decrypter for DumEnc {
    fn decrypt_block(&self, block: &mut [u8]) {
        for b in block {
            *b = b.wrapping_sub(0x5a);
        }
    }
}

struct DumMac(u8);
impl Mac for DumMac {
    fn input(&mut self, data: &[u8]) {
        for b in data {
            self.0 ^= *b;
        }
    }
    fn reset(&mut self) {
        self.0 = 0;
    }
    fn result(self) -> [u8; 16] {
        [self.0; 16]
    }
}

struct DumFactory;
impl CryptoFactory for DumFactory {
    type E = DumEnc;
    type D = DumEnc;
    type M = DumMac;
    fn new_enc(&self, _: &AES128) -> DumEnc {
        DumEnc
    }
    fn new_dec(&self, _: &AES128) -> DumEnc {
        DumEnc
    }
    fn new_mac(&self, _: &AES128) -> DumMac {
        DumMac(0)
    }
}

const KEY: AES128 = AES128([2; 16]);

// --- Old API -------------------------------------------------------------

#[cfg(any(feature = "old1", feature = "old3"))]
mod old {
    use super::*;
    use lorawan::parser::{
        parse, DataHeader, DataPayload, FRMPayload, JoinAcceptPayload, MICAble, PhyPayload,
    };

    pub fn work<T: AsRef<[u8]> + AsMut<[u8]>>(data: T) -> u32 {
        match parse(data) {
            Ok(PhyPayload::Data(DataPayload::Encrypted(p))) => {
                let mut acc = u32::from(p.fhdr().fcnt());
                acc = acc.wrapping_add(u32::from(p.fhdr().dev_addr().as_ref()[0]));
                acc = acc.wrapping_add(u32::from(p.f_port().unwrap_or(0)));
                if p.validate_mic(&KEY, 1, &DumFactory) {
                    acc = acc.wrapping_add(1);
                }
                if let Ok(d) = p.decrypt(Some(&KEY), Some(&KEY), 1, &DumFactory) {
                    if let FRMPayload::Data(pl) = d.frm_payload() {
                        for b in pl {
                            acc = acc.wrapping_add(u32::from(*b));
                        }
                    }
                }
                acc
            }
            Ok(PhyPayload::JoinRequest(jr)) => {
                let mut acc = u32::from(jr.dev_eui().as_ref()[0]);
                if jr.validate_mic(&KEY, &DumFactory) {
                    acc = acc.wrapping_add(1);
                }
                acc
            }
            Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(ja))) => {
                let d = ja.decrypt(&AppKey::from(KEY.0), &DumFactory);
                let mut acc = u32::from(d.dev_addr().as_ref()[0]);
                acc = acc.wrapping_add(u32::from(d.mic().0[0]));
                acc
            }
            _ => 0,
        }
    }
}

#[cfg(any(feature = "old1", feature = "old3"))]
#[no_mangle]
pub unsafe extern "C" fn work_slice(ptr: *mut u8, len: usize) -> u32 {
    old::work(core::slice::from_raw_parts_mut(ptr, len))
}

#[cfg(feature = "old3")]
#[no_mangle]
pub unsafe extern "C" fn work_array(ptr: *const u8) -> u32 {
    let mut data = [0u8; 33];
    core::ptr::copy_nonoverlapping(ptr, data.as_mut_ptr(), 33);
    old::work(data)
}

#[cfg(feature = "old3")]
pub struct Wrapper([u8; 33]);
#[cfg(feature = "old3")]
impl AsRef<[u8]> for Wrapper {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
#[cfg(feature = "old3")]
impl AsMut<[u8]> for Wrapper {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

#[cfg(feature = "old3")]
#[no_mangle]
pub unsafe extern "C" fn work_wrapper(ptr: *const u8) -> u32 {
    let mut data = [0u8; 33];
    core::ptr::copy_nonoverlapping(ptr, data.as_mut_ptr(), 33);
    old::work(Wrapper(data))
}

// --- New API -------------------------------------------------------------

#[cfg(any(feature = "new1", feature = "new3"))]
mod new {
    use super::*;
    use lorawan::v2::{
        parse, DecryptedDataPayload, DecryptedJoinAcceptPayload, FrmPayload, PhyPayload,
    };

    pub fn work(buf: &mut [u8]) -> u32 {
        match parse(buf) {
            Ok(PhyPayload::Data(p)) => {
                let mut acc = u32::from(p.fhdr().fcnt());
                acc = acc.wrapping_add(p.fhdr().dev_addr().value());
                acc = acc.wrapping_add(u32::from(p.f_port().unwrap_or(0)));
                if p.validate_mic(&KEY, 1, &DumFactory) {
                    acc = acc.wrapping_add(1);
                }
                let nwk = lorawan::keys::NwkSKey::from(KEY.0);
                let app = lorawan::keys::AppSKey::from(KEY.0);
                if let Ok(d) = DecryptedDataPayload::decrypt_in_place(
                    buf,
                    Some(&nwk),
                    Some(&app),
                    1,
                    &DumFactory,
                ) {
                    if let FrmPayload::Data(pl) = d.frm_payload() {
                        for b in pl {
                            acc = acc.wrapping_add(u32::from(*b));
                        }
                    }
                }
                acc
            }
            Ok(PhyPayload::JoinRequest(jr)) => {
                let mut acc = jr.dev_eui().value() as u32;
                if jr.validate_mic(&AppKey::from(KEY.0), &DumFactory) {
                    acc = acc.wrapping_add(1);
                }
                acc
            }
            Ok(PhyPayload::JoinAccept(_)) => {
                let key = AppKey::from(KEY.0);
                match DecryptedJoinAcceptPayload::decrypt_in_place(buf, &key, &DumFactory) {
                    Ok(d) => d.dev_addr().value().wrapping_add(u32::from(d.mic().0[0])),
                    Err(_) => 0,
                }
            }
            _ => 0,
        }
    }
}

#[cfg(any(feature = "new1", feature = "new3"))]
#[no_mangle]
pub unsafe extern "C" fn work_slice_new(ptr: *mut u8, len: usize) -> u32 {
    new::work(core::slice::from_raw_parts_mut(ptr, len))
}

#[cfg(feature = "new3")]
#[no_mangle]
pub unsafe extern "C" fn work_array_new(ptr: *const u8) -> u32 {
    let mut data = [0u8; 33];
    core::ptr::copy_nonoverlapping(ptr, data.as_mut_ptr(), 33);
    new::work(&mut data)
}

#[cfg(feature = "new3")]
#[no_mangle]
pub unsafe extern "C" fn work_wrapper_new(ptr: *const u8) -> u32 {
    let mut data = [0u8; 33];
    core::ptr::copy_nonoverlapping(ptr, data.as_mut_ptr(), 33);
    // Same slice implementation; a wrapper type adds no monomorphization.
    new::work(&mut data[..])
}
