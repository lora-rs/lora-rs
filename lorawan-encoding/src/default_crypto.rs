//! Provides a default software implementation for LoRaWAN's cryptographic functions.
use super::keys::*;
use aes::cipher::{
    BlockCipherDecrypt as BlockDecrypt, BlockCipherEncrypt as BlockEncrypt, KeyInit,
};
use aes::{Aes128, Aes128Enc};
use cmac::digest::InnerInit;
use cmac::Cmac as RustCmac;

pub type Cmac = RustCmac<Aes128>;

/// Default software implementation of the device-side [`Crypto`] primitives.
///
/// Holds the expanded AES key schedule for the key it was constructed with,
/// so repeated operations under the same key (every frame of a session) skip
/// the key expansion. Only the encrypt schedule is kept; a device never needs
/// the decrypt primitive.
#[derive(Clone)]
pub struct DefaultCrypto {
    cipher: Aes128Enc,
}

impl DefaultCrypto {
    pub fn new(key: &AES128) -> Self {
        Self { cipher: Aes128Enc::new_from_slice(&key.0[..]).unwrap() }
    }
}

impl From<AES128> for DefaultCrypto {
    fn from(key: AES128) -> Self {
        Self::new(&key)
    }
}

impl core::fmt::Debug for DefaultCrypto {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DefaultCrypto { .. }")
    }
}

impl Crypto for DefaultCrypto {
    fn encrypt_block(&self, block: &mut [u8]) {
        BlockEncrypt::encrypt_block(&self.cipher, block.try_into().unwrap());
    }

    fn calculate_mic(&self, b0: &[u8], data: &[u8]) -> [u8; 4] {
        calculate_mic(self.cipher.clone(), b0, data)
    }
}

/// Default software implementation of the network-side [`NetworkCrypto`]
/// primitives, holding both the encrypt and decrypt AES key schedules.
#[derive(Clone)]
pub struct DefaultNetworkCrypto {
    cipher: Aes128,
}

impl DefaultNetworkCrypto {
    pub fn new(key: &AES128) -> Self {
        Self { cipher: Aes128::new_from_slice(&key.0[..]).unwrap() }
    }
}

impl From<AES128> for DefaultNetworkCrypto {
    fn from(key: AES128) -> Self {
        Self::new(&key)
    }
}

impl core::fmt::Debug for DefaultNetworkCrypto {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DefaultNetworkCrypto { .. }")
    }
}

impl Crypto for DefaultNetworkCrypto {
    fn encrypt_block(&self, block: &mut [u8]) {
        BlockEncrypt::encrypt_block(&self.cipher, block.try_into().unwrap());
    }

    fn calculate_mic(&self, b0: &[u8], data: &[u8]) -> [u8; 4] {
        calculate_mic(self.cipher.clone(), b0, data)
    }
}

impl NetworkCrypto for DefaultNetworkCrypto {
    fn decrypt_block(&self, block: &mut [u8]) {
        BlockDecrypt::decrypt_block(&self.cipher, block.try_into().unwrap());
    }
}

fn calculate_mic<C>(cipher: C, b0: &[u8], data: &[u8]) -> [u8; 4]
where
    C: cmac::block_api::CmacCipher,
{
    // CMAC subkeys are derived from the cipher at finalization, so
    // initializing from the cached key schedule skips the key expansion.
    let mut mac = RustCmac::inner_init(cipher);
    cmac::Mac::update(&mut mac, b0);
    cmac::Mac::update(&mut mac, data);
    let result = cmac::Mac::finalize(mac).into_bytes();
    result[0..4].try_into().unwrap()
}

#[cfg(test)]
mod test {
    use super::*;

    // NIST SP 800-38B / RFC 4493 test key and vectors.
    const KEY: AES128 = AES128([
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ]);
    const PLAINTEXT: [u8; 16] = [
        0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93, 0x17,
        0x2a,
    ];
    const CIPHERTEXT: [u8; 16] = [
        0x3a, 0xd7, 0x7b, 0xb4, 0x0d, 0x7a, 0x36, 0x60, 0xa8, 0x9e, 0xca, 0xf3, 0x24, 0x66, 0xef,
        0x97,
    ];

    #[test]
    fn encrypt_block_known_answer() {
        for crypto in [
            &DefaultCrypto::new(&KEY) as &dyn Crypto,
            &DefaultNetworkCrypto::new(&KEY) as &dyn Crypto,
        ] {
            let mut block = PLAINTEXT;
            crypto.encrypt_block(&mut block);
            assert_eq!(block, CIPHERTEXT);
        }
    }

    #[test]
    fn decrypt_block_inverts_encrypt() {
        let crypto = DefaultNetworkCrypto::new(&KEY);
        let mut block = CIPHERTEXT;
        crypto.decrypt_block(&mut block);
        assert_eq!(block, PLAINTEXT);
    }

    #[test]
    fn calculate_mic_known_answer() {
        // RFC 4493 example 2: 16-byte message, full tag starts with
        // 070a16b4 6b4d4144.
        for crypto in [
            &DefaultCrypto::new(&KEY) as &dyn Crypto,
            &DefaultNetworkCrypto::new(&KEY) as &dyn Crypto,
        ] {
            assert_eq!(crypto.calculate_mic(&[], &PLAINTEXT), [0x07, 0x0a, 0x16, 0xb4]);
            // The MIC over split input must equal the MIC over the
            // concatenation.
            assert_eq!(
                crypto.calculate_mic(&PLAINTEXT[..7], &PLAINTEXT[7..]),
                [0x07, 0x0a, 0x16, 0xb4]
            );
        }
    }
}

/// Provides a default implementation for build object for using the crypto functions.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct DefaultFactory;

impl CryptoFactory for DefaultFactory {
    type E = Aes128;
    type D = Aes128;
    type M = Cmac;

    fn new_enc(&self, key: &AES128) -> Self::E {
        Self::E::new_from_slice(&key.0[..]).unwrap()
    }

    fn new_dec(&self, key: &AES128) -> Self::D {
        Self::D::new_from_slice(&key.0[..]).unwrap()
    }

    fn new_mac(&self, key: &AES128) -> Self::M {
        Self::M::new_from_slice(&key.0[..]).unwrap()
    }
}

impl Encrypter for Aes128 {
    fn encrypt_block(&self, block: &mut [u8]) {
        BlockEncrypt::encrypt_block(self, block.try_into().unwrap());
    }
}

impl Decrypter for Aes128 {
    fn decrypt_block(&self, block: &mut [u8]) {
        BlockDecrypt::decrypt_block(self, block.try_into().unwrap());
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
