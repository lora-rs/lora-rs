//! Provides a default software implementation for LoRaWAN's cryptographic functions.
use super::keys::*;
use aes::cipher::{
    Array as GenericArray, BlockCipherDecrypt as BlockDecrypt, BlockCipherEncrypt as BlockEncrypt,
    KeyInit,
};
use aes::Aes128;
use cmac::Cmac as RustCmac;

pub type Cmac = RustCmac<Aes128>;

/// Provides a default implementation for build object for using the crypto functions.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct DefaultFactory;

impl CryptoFactory for DefaultFactory {
    type E = Aes128;
    type D = Aes128;
    type M = Cmac;

    fn new_enc(&self, key: &AES128) -> Self::E {
        Aes128::new(GenericArray::from_slice(&key.0[..]))
    }

    fn new_dec(&self, key: &AES128) -> Self::D {
        Aes128::new(GenericArray::from_slice(&key.0[..]))
    }

    fn new_mac(&self, key: &AES128) -> Self::M {
        let key = GenericArray::from_slice(&key.0[..]);
        Cmac::new(key)
    }
}

impl Encrypter for Aes128 {
    fn encrypt_block(&self, block: &mut [u8]) {
        BlockEncrypt::encrypt_block(self, GenericArray::from_mut_slice(block));
    }
}

impl Decrypter for Aes128 {
    fn decrypt_block(&self, block: &mut [u8]) {
        BlockDecrypt::decrypt_block(self, GenericArray::from_mut_slice(block));
    }
}

impl Mac for Cmac {
    fn input(&mut self, data: &[u8]) {
        cmac::Mac::update(self, data);
    }

    fn reset(&mut self) {
        cmac::Mac::reset(self);
    }

    fn result(self) -> [u8; 16] {
        cmac::Mac::finalize(self).into_bytes().into()
    }
}
