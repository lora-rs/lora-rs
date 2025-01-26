//! Implement types for dealing with LoRaWAN keys and required
//! cryptography entities.
use super::parser::{McAddr, EUI64};

macro_rules! lorawan_key {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(AES128);
    ) => {
        $(#[$outer])*
        #[doc = concat!(
            "# Usage\n\n",
            "## Creating from a hex-encoded MSB string:\n",
            "```\n",
            "use lorawan::keys::", stringify!($type), ";\n",
            "use core::str::FromStr;\n",
            "let key = ", stringify!($type), "::from_str(\"00112233445566778899aabbccddeeff\").unwrap();\n",
            "```\n\n",
            "## Creating from a byte array in MSB format:\n",
            "```\n",
            "use lorawan::keys::", stringify!($type), ";\n",
            "let key = ", stringify!($type), "::from([\n",
            "    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF\n",
            "]);\n",
            "```\n"
        )]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
        pub struct $type(pub(crate) AES128);

        impl $type {
            pub const fn byte_len() -> usize {
                16
            }
        }

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
    pub struct AppKey(AES128);
);
lorawan_key!(
    /// The [`AppSKey`] is an application session key (AES-128) specific to
    /// the end-device.
    ///
    /// `AppSKey` SHOULD be stored such that extraction and re-use by malicious
    /// actors is prevented.
    pub struct AppSKey(AES128);
);

lorawan_key!(
    /// The [`NwkSKey`] is a network session key (AES-128) specific to the end-device.
    ///
    /// It SHOULD be stored such that extraction and re-use by malicious
    /// actors is prevented.
    pub struct NwkSKey(AES128);
);

#[deprecated(since = "0.9.1", note = "Please use `NwkSKey` instead")]
pub type NewSKey = NwkSKey;

lorawan_key!(
    pub struct McKey(AES128);
);
/// The [`AppKey`] is an AES-128 root key specific to the end-device.
///
/// `AppKey` SHALL be stored on an end-device intending to use the OTAA procedure.
/// It is NOT REQUIRED for ABP-only end-devices.
impl McKey {
    /// McAppSKey = aes128_encrypt(McKey, 0x01 | McAddr | pad16)
    pub fn derive_mc_app_s_key<F: CryptoFactory, T: AsRef<[u8]>>(
        &self,
        crypto: &F,
        mc_addr: &McAddr<T>,
    ) -> McAppSKey {
        let aes_enc = crypto.new_enc(&self.0);
        let mut bytes: [u8; 16] = [0; 16];
        bytes[0] = 0x01;
        bytes[1..5].copy_from_slice(mc_addr.as_ref());
        aes_enc.encrypt_block(&mut bytes);
        McAppSKey::from(bytes)
    }

    /// McNetSKey = aes128_encrypt(McKey, 0x02 | McAddr | pad16)
    pub fn derive_mc_net_s_key<F: CryptoFactory, T: AsRef<[u8]>>(
        &self,
        crypto: &F,
        mc_addr: &McAddr<T>,
    ) -> McNetSKey {
        let aes_enc = crypto.new_enc(&self.0);
        let mut bytes: [u8; 16] = [0; 16];
        bytes[0] = 0x02;
        bytes[1..5].copy_from_slice(mc_addr.as_ref());
        aes_enc.encrypt_block(&mut bytes);
        McNetSKey::from(bytes)
    }
}

lorawan_key!(
    /// The multicast network session key ([`McNetSKey`]) is an application session key (AES-128)
    /// for a multicast group.
    ///
    /// `McNetSKey` SHOULD be stored such that extraction and re-use by malicious
    /// actors is prevented.
    pub struct McNetSKey(AES128);
);

lorawan_key!(
    /// The multicast application session key ([`McAppSKey`]) is an application session key (AES-128)
    /// for a multicast group.
    ///
    /// `McNetSKey` SHOULD be stored such that extraction and re-use by malicious
    /// actors is prevented.
    pub struct McAppSKey(AES128);
);

lorawan_key!(
    /// The multicast root key ([`McRootKey`]) is an AES-128 key specific to the end-device. For
    /// Lorawan 1.0.x, it is derived by the "GenAppKey" ([`GenAppKey`]). For Lorawan 1.1.x, it is
    /// derived by the "AppKey" ([`AppKey`]) or the "GenMcKey" ([`McKey`]).
    ///
    /// `McRootKey` SHALL be stored on an end-device intended to be used for deriving the
    /// [`McKEKey`].
    pub struct McRootKey(AES128);
);

lorawan_key!(
    /// The multicast key encryption key ([`McKEKey`]) is an AES-128 key specific to the end-device,
    /// derived from the `McRootKey`.
    ///
    /// The McKEKey is a lifetime end-device specific key.
    pub struct McKEKey(AES128);
);

lorawan_key!(
    /// The [`GenAppKey`] is an AES-128 root key specific to the end-device.
    ///
    /// `GenAppKey` SHALL be stored on an end-device intending to be used to derive the McRootKey.
    /// It is NOT REQUIRED for ABP-only end-devices.
    pub struct GenAppKey(AES128);
);

impl McKEKey {
    /// McKEKey = aes128_encrypt(McRootKey, 0x00 | pad16)
    pub fn derive_from<F: CryptoFactory>(crypto: &F, root_key: &McRootKey) -> Self {
        let aes_enc = crypto.new_enc(&root_key.0);
        let mut bytes: [u8; 16] = [0; 16];
        aes_enc.encrypt_block(&mut bytes);
        McKEKey::from(bytes)
    }
}

impl McRootKey {
    /// LoRaWAN 1.1.x: McRootKey = aes128_encrypt(AppKey, 0x20 | pad16)
    pub fn derive_from_app_key<F: CryptoFactory>(crypto: &F, app_key: &AppKey) -> Self {
        let aes_enc = crypto.new_enc(&app_key.0);
        let mut bytes: [u8; 16] = [0; 16];
        bytes[0] = 0x20;
        aes_enc.encrypt_block(&mut bytes);
        McRootKey::from(bytes)
    }

    /// LoRaWAN 1.0.x: McRootKey = aes128_encrypt(GenAppKey, 0x00 | pad16)
    pub fn derive_from_gen_app_key<F: CryptoFactory>(crypto: &F, app_key: &GenAppKey) -> Self {
        let aes_enc = crypto.new_enc(&app_key.0);
        let mut bytes: [u8; 16] = [0; 16];
        aes_enc.encrypt_block(&mut bytes);
        McRootKey::from(bytes)
    }
}

macro_rules! lorawan_eui {
    (
        $(#[$outer:meta])*
        pub struct $type:ident(EUI64<[u8; 8]>);
    ) => {
        $(#[$outer])*
        #[doc = concat!(
            "# Usage\n\n",
            "## Creating from a hex-encoded LSB string:\n",
            "```\n",
            "use lorawan::keys::", stringify!($type), ";\n",
            "use core::str::FromStr;\n",
            "let eui = ", stringify!($type), "::from_str(\"0011223344556677\").unwrap();\n",
            "```\n\n",
            "## Creating from a byte array in LSB format:\n",
            "```\n",
            "use lorawan::keys::", stringify!($type), ";\n",
            "let eui = ", stringify!($type), "::from([\n",
            "    0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00\n",
            "]);\n",
            "```\n"
        )]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
        pub struct $type(EUI64<[u8; 8]>);

        impl $type {
            pub const fn byte_len() -> usize {
                8
            }
        }

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::default_crypto::DefaultFactory;

    const TEST_KEY: [u8; 16] = [4, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

    const ADDR: [u8; 4] = [1, 2, 3, 4];
    #[test]
    fn mc_root_key_to_mc_ke_key() {
        let mc_root_key = McRootKey::from(TEST_KEY);
        let mc_ke_key = McKEKey::derive_from(&DefaultFactory, &mc_root_key);
        assert_eq!(
            McKEKey(AES128([
                0x90, 0x83, 0xbe, 0xbf, 0x70, 0x42, 0x57, 0x88, 0x31, 0x60, 0xdb, 0xfc, 0xde, 0x33,
                0xad, 0x71
            ])),
            mc_ke_key
        )
    }

    #[test]
    fn mc_key_to_mc_app_s_key() {
        let mc_key = McKey::from(TEST_KEY);
        let mc_app_s_key = mc_key.derive_mc_app_s_key(&DefaultFactory, &McAddr::from(ADDR));
        assert_eq!(
            McAppSKey(AES128([
                0x50, 0xDF, 0x70, 0x27, 0xEF, 0xC6, 0xB4, 0x7D, 0xA8, 0x10, 0xEE, 0x3C, 0xCA, 0x0D,
                0x15, 0xAF
            ])),
            mc_app_s_key
        )
    }

    #[test]
    fn mc_key_to_mc_net_s_key() {
        let mc_key = McKey::from(TEST_KEY);
        let mc_net_s_key = mc_key.derive_mc_net_s_key(&DefaultFactory, &McAddr::from(ADDR));
        assert_eq!(
            McNetSKey(AES128([
                0x8D, 0xF7, 0x07, 0x27, 0x36, 0x47, 0xE2, 0x2E, 0x4E, 0x27, 0xFE, 0x00, 0x4B, 0x99,
                0x52, 0xBF
            ])),
            mc_net_s_key
        )
    }
}
