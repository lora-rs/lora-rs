// Copyright (c) 2017-2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use generic_array::{GenericArray, typenum::U16};
use aes::Aes128;
use aes::block_cipher_trait::BlockCipher;

/// AES128 represents 128 bit AES key.
#[derive(Debug, Default, PartialEq)]
pub struct AES128(pub [u8; 16]);

impl From<[u8; 16]> for AES128 {
    fn from(v: [u8; 16]) -> Self {
        AES128(v)
    }
}

/// MIC represents LoRaWAN MIC.
#[derive(Debug, Default, PartialEq)]
pub struct MIC(pub [u8; 4]);

impl From<[u8; 4]> for MIC {
    fn from(v: [u8; 4]) -> Self {
        MIC(v)
    }
}

/// Trait for implementations of AES128 encryption.
pub trait Encrypter {
    fn encrypt_block(&self, block: &mut GenericArray<u8, U16>);
}

/// Trait for implementations of AES128 decryption.
pub trait Decrypter {
    fn decrypt_block(&self, block: &mut GenericArray<u8, U16>);
}

/// Trait for implementations of CMAC.
pub trait Mac {
    fn input(&mut self, data: &[u8]);
    fn reset(&mut self);
    fn result(self) -> GenericArray<u8, U16>;
}

/// Represents an abstraction over the crypto functions.
///
/// This trait provides a way to pick a different implementation of the crypto primitives.
pub trait CryptoFactory {
    type E: Encrypter;
    type D: Decrypter;
    type M: Mac;

    /// Method that creates an Encrypter.
    fn new_enc(&self, key: &AES128) -> Self::E;

    /// Method that creates a Decrypter.
    fn new_dec(&self, key: &AES128) -> Self::D;

    /// Method that creates a MAC calculator.
    fn new_mac(&self, key: &AES128) -> Self::M;
}

pub type Cmac = cmac::Cmac<Aes128>;

/// Provides a default implementation for build object for using the crypto functions.
#[derive(Default,Debug, PartialEq)]
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
        use cmac::Mac;
        Cmac::new_varkey(&key.0[..]).unwrap()
    }
}

impl Encrypter for Aes128 {
    fn encrypt_block(&self, block: &mut GenericArray<u8, U16>) {
        aes::block_cipher_trait::BlockCipher::encrypt_block(self, block);
    }
}

impl Decrypter for Aes128 {
    fn decrypt_block(&self, block: &mut GenericArray<u8, U16>) {
        aes::block_cipher_trait::BlockCipher::decrypt_block(self, block);
    }
}

impl Mac for Cmac {
    fn input(&mut self, data: &[u8]) {
        cmac::Mac::input(self, data);
    }

    fn reset(&mut self) {
        cmac::Mac::reset(self);
    }

    fn result(self) -> GenericArray<u8, U16> {
        cmac::Mac::result(self).code()
    }
}
