// Copyright (c) 2017-2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>
use super::parser::EUI64;
use super::securityhelpers::generic_array::{typenum::U16, GenericArray};

macro_rules! lorawan_key {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(AES128);
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $type(pub(crate) AES128);

        impl From<[u8;16]> for $type {
            fn from(key: [u8; 16]) -> Self {
                $type(AES128(key))
            }
        }

        impl $type {
            pub fn inner(&self) -> &AES128 {
                &self.0
            }
        }

        impl AsRef<[u8]> for $type {
            fn as_ref(&self) -> &[u8] {
                &self.0 .0
            }
        }
    };
}

lorawan_key!(
    /// AppKey should be entered in MSB format. For example, if your LNS provides a AppKey of
    /// `00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF`, you should enter it as `AppKey([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])`.
    /// Alternatively, you can use the from_str method
    pub struct AppKey(AES128);
);
lorawan_key!(
    /// NwkSKey should be entered in MSB format. For example, if your LNS provides a NwkSKey of
    /// `00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF`, you should enter it as `NwkSKey([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])`.
    pub struct NewSKey(AES128);
);
lorawan_key!(
    /// AppSKey should be entered in MSB format. For example, if your LNS provides a AppSKey of
    /// `00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF`, you should enter it as `AppSKey([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])`.
    pub struct AppSKey(AES128);
);

macro_rules! lorawan_eui {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(EUI64<[u8; 8]>);
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $type(EUI64<[u8; 8]>);

        impl From<[u8;8]> for $type {
            fn from(key: [u8; 8]) -> Self {
                $type(EUI64::from(key))
            }
        }

        impl From<$type> for EUI64<[u8; 8]> {
            fn from(key: $type) -> Self {
                key.0
            }
        }

        impl AsRef<[u8]> for $type {
            fn as_ref(&self) -> &[u8] {
                &self.0.as_ref()
            }
        }
    };
}

lorawan_eui!(
    /// DevEui should be entered in LSB format. For example, if your LNS provides a DevEui of
    /// `00:11:22:33:44:55:66:77`, you should enter it as `DevEui([0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00])`.
    pub struct DevEui(EUI64<[u8; 8]>);
);
lorawan_eui!(
    /// AppEui should be entered in LSB format. For example, if your LNS provides a AppEui of
    /// `00:11:22:33:44:55:66:77`, you should enter it as `AppEui([0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00])`.
    pub struct AppEui(EUI64<[u8; 8]>);
);

/// AES128 represents 128-bit AES key.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct AES128(pub [u8; 16]);

impl From<[u8; 16]> for AES128 {
    fn from(v: [u8; 16]) -> Self {
        AES128(v)
    }
}

/// MIC represents LoRaWAN MIC.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
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
