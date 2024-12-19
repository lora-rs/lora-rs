//! Implement types for dealing with LoRaWAN keys and required
//! cryptography entities.
use super::parser::EUI64;

macro_rules! lorawan_key {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(AES128);
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
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
    /// The [`AppKey`] is an AES-128 root key specific to the end-device.
    ///
    /// `AppKey` SHALL be stored on an end-device intending to use the OTAA procedure.
    /// It is NOT REQUIRED for ABP-only end-devices.
    ///
    /// To create from a hex-encoded MSB string:
    /// ```
    /// use lorawan::keys::AppKey;
    /// use core::str::FromStr;
    ///let appkey = AppKey::from_str("00112233445566778899aabbccddeeff").unwrap();
    /// ```
    ///
    /// To create from a byte array, you should enter the bytes in MSB format:
    /// ```
    /// use lorawan::keys::AppKey;
    /// let appkey = AppKey::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    /// ```
    pub struct AppKey(AES128);
);
lorawan_key!(
    /// The [`AppSKey`] is an application session key (AES-128) specific to
    /// the end-device.
    ///
    /// `AppSKey` SHOULD be stored such that extraction and re-use by malicious
    /// actors is prevented.
    ///
    /// To create from a hex-encoded MSB string:
    /// ```
    /// use lorawan::keys::AppSKey;
    /// use core::str::FromStr;
    /// let appskey = AppSKey::from_str("00112233445566778899aabbccddeeff").unwrap();
    /// ```
    ///
    /// To create from a byte array, you should enter the bytes in MSB format:
    /// ```
    /// use lorawan::keys::AppSKey;
    /// let appskey = AppSKey::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    /// ```
    pub struct AppSKey(AES128);
);

lorawan_key!(
    /// The [`NwkSKey`] is a network session key (AES-128) specific to the end-device.
    ///
    /// It SHOULD be stored such that extraction and re-use by malicious
    /// actors is prevented.
    ///
    /// To create from a hex-encoded MSB string:
    /// ```
    /// use lorawan::keys::NwkSKey;
    /// use core::str::FromStr;
    /// let nwkskey = NwkSKey::from_str("00112233445566778899aabbccddeeff").unwrap();
    /// ```
    ///
    /// To create from a byte array, you should enter the bytes in MSB format:
    /// ```
    /// use lorawan::keys::NwkSKey;
    /// let nwkskey = NwkSKey::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    /// ```
    pub struct NwkSKey(AES128);
);

#[deprecated(since = "0.9.1", note = "Please use `NwkSKey` instead")]
pub type NewSKey = NwkSKey;

macro_rules! lorawan_eui {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(EUI64<[u8; 8]>);
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
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
    /// [`DevEui`] is a global end-device ID in the IEEE EUI64 address space
    /// that uniquely identifies the end-device across roaming networks.
    ///
    /// All end-devices SHALL have an assigned `DevEui` regardless of which
    /// activation procedure is used (i.e., ABP or OTAA).
    ///
    /// It is a recommended practice that `DevEui` should also be available on
    /// an end-device label for the purpose of end-device administration.
    ///
    /// To create from a hex-encoded LSB string:
    /// ```
    /// use lorawan::keys::DevEui;
    /// use core::str::FromStr;
    /// let dev_eui = DevEui::from_str("0011223344556677").unwrap();
    /// ```
    ///
    /// To create from a byte array, you should enter the bytes in LSB format:
    /// ```
    /// use lorawan::keys::DevEui;
    /// let dev_eui = DevEui::from([0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00]);
    /// ```
    pub struct DevEui(EUI64<[u8; 8]>);
);
lorawan_eui!(
    /// The [`AppEui`] is a global application ID in IEEE EUI64 address space
    /// that uniquely identifies the entity able to process the JoinReq frame.
    ///
    /// For OTAA end-devices, `AppEui` SHALL be stored in the end-device before
    /// the Join procedure is executed, although some network servers ignore
    /// this value.
    /// `AppEui` is not required for ABP-only end-devices.
    ///
    /// As of LoRaWAN 1.0.4, `AppEui` is called `JoinEui`.
    ///
    /// To create from a hex-encoded LSB string:
    /// ```
    /// use lorawan::keys::AppEui;
    /// use core::str::FromStr;
    /// let app_eui = AppEui::from_str("0011223344556677").unwrap();
    /// ```
    ///
    /// To create from a byte array, you should enter the bytes in LSB format:
    /// ```
    /// use lorawan::keys::AppEui;
    /// let app_eui = AppEui::from([0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00]);
    /// ```
    pub struct AppEui(EUI64<[u8; 8]>);
);

/// [`AES128`] represents 128-bit AES key.
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct AES128(pub [u8; 16]);

impl From<[u8; 16]> for AES128 {
    fn from(v: [u8; 16]) -> Self {
        AES128(v)
    }
}

/// [`MIC`] represents LoRaWAN message integrity code (MIC).
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct MIC(pub [u8; 4]);

impl From<[u8; 4]> for MIC {
    fn from(v: [u8; 4]) -> Self {
        MIC(v)
    }
}

/// Trait for implementations of AES128 encryption.
pub trait Encrypter {
    fn encrypt_block(&self, block: &mut [u8]);
}

/// Trait for implementations of AES128 decryption.
pub trait Decrypter {
    fn decrypt_block(&self, block: &mut [u8]);
}

/// Trait for implementations of CMAC (RFC4493).
pub trait Mac {
    fn input(&mut self, data: &[u8]);
    fn reset(&mut self);
    fn result(self) -> [u8; 16];
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
