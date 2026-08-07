//! Flash-size harness for the borrowed-view parser/creator API, on
//! thumbv7em-none-eabi.
//!
//! Each feature selects one configuration; all do the same work (parse a
//! data frame, read header fields, validate MIC, decrypt, fold the payload,
//! plus the join path) through a dummy crypto factory so that AES tables
//! don't drown out the parser code being measured:
//!
//! * `new1`: one entry point (`&mut [u8]`)
//! * `new3`: three entry points (slice, owned array, wrapped array), all
//!   funneling into the single slice-based implementation
//!
//! The commit that introduced this API ran this same workload through the old
//! `T: AsRef<[u8]>` API side by side; see the README there for the
//! comparison table.

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

#[cfg(any(feature = "new1", feature = "new3"))]
mod new {
    use super::*;
    use lorawan::parser::{parse, DecryptedDataPayload, DecryptedJoinAcceptPayload, FrmPayload, PhyPayload};

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
