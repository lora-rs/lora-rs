//! Provides types and methods for parsing LoRaWAN payloads.
//!
//! # Examples
//!
//! ```
//! use lorawan::parser::*;
//! use lorawan::keys::*;
//!
//! let data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01,
//!     0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6, 0xc3, 0xb5, 0x82];
//! if let Ok(PhyPayload::Data(DataPayload::Encrypted(phy))) = parse(data) {
//!     let key = AES128([1; 16]);
//!     let decrypted = phy.decrypt(None, Some(&key), 1).unwrap();
//!     if let FRMPayload::Data(data_payload) =
//!             decrypted.frm_payload() {
//!         println!("{}", String::from_utf8_lossy(data_payload));
//!     }
//! } else {
//!     panic!("failed to parse data payload");
//! }
//! ```

use crate::keys::NewSKey;

use super::keys::{AppKey, AppSKey, CryptoFactory, Encrypter, AES128, MIC};
use super::maccommands::{ChannelMask, DLSettings, Frequency};

use super::securityhelpers;
use super::securityhelpers::generic_array::GenericArray;

use super::packet_length::phy::{join::*, mac::FPORT_LEN, MHDR_LEN, MIC_LEN, PHY_PAYLOAD_MIN_LEN};

#[cfg(feature = "default-crypto")]
use super::default_crypto::DefaultFactory;

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    InvalidData,
    InvalidMic,
    InvalidKey,
    InvalidMessageType,
    InvalidPayload,
    UnsupportedMajorVersion,
}

macro_rules! fixed_len_struct {
    (
        $(#[$outer:meta])*
        struct $type:ident[$size:expr];
    ) => {
        $(#[$outer])*
        #[derive(Debug, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $type<T: AsRef<[u8]>>(T);

        impl<T: AsRef<[u8]>> $type<T> {
            fn new_from_raw(bytes: T) -> $type<T> {
                $type(bytes)
            }

            pub fn new(data: T) -> Option<$type<T>> {
                let bytes = data.as_ref();
                if bytes.len() != $size {
                    None
                } else {
                    Some($type(data))
                }
            }
        }

        impl<T: AsRef<[u8]> + Clone> Clone for $type<T> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }

        impl<T: AsRef<[u8]> + Copy> Copy for $type<T> {
        }

        impl<T: AsRef<[u8]>, V: AsRef<[u8]>> PartialEq<$type<T>> for $type<V> {
            fn eq(&self, other: &$type<T>) -> bool {
                self.as_ref() == other.as_ref()
            }
        }

        impl<'a> From<&'a [u8; $size]> for $type<&'a [u8; $size]> {
            fn from(v: &'a [u8; $size]) -> Self {
                $type(v)
            }
        }

        impl From<[u8; $size]> for $type<[u8; $size]> {
            fn from(v: [u8; $size]) -> Self {
                $type(v)
            }
        }

        impl<T: AsRef<[u8]>> AsRef<[u8]> for $type<T> {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }

        impl<T: AsRef<[u8]>> $type<T> {
            #[inline]
            pub fn to_owned(&self) -> $type<[u8; $size]> {
                let mut data = [0 as u8; $size];
                data.copy_from_slice(self.0.as_ref());
                $type(data)
            }
        }

        impl<T: AsRef<[u8]> + Default> Default for $type<T> {
            #[inline]
            fn default() -> $type<T> {
                $type(T::default())
            }
        }

    };
}

/// PhyPayload is a type that represents a physical LoRaWAN payload.
///
/// It can either be JoinRequest, JoinAccept, or DataPayload.
#[derive(Debug, PartialEq, Eq)]
pub enum PhyPayload<T, F> {
    JoinRequest(JoinRequestPayload<T, F>),
    JoinAccept(JoinAcceptPayload<T, F>),
    Data(DataPayload<T, F>),
}

#[cfg(feature = "defmt")]
impl<T: defmt::Format, F> defmt::Format for PhyPayload<T, F> {
    fn format(&self, f: defmt::Formatter<'_>) {
        match self {
            PhyPayload::JoinRequest(r) => {
                defmt::write!(f, "JoinRequestPayload({})", r.0);
            }
            PhyPayload::JoinAccept(a) => match a {
                JoinAcceptPayload::Encrypted(data) => {
                    defmt::write!(f, "JoinAcceptPayload::Encrypted({})", data.0);
                }
                JoinAcceptPayload::Decrypted(data) => {
                    defmt::write!(f, "JoinAcceptPayload::Decrypted({})", data.0);
                }
            },
            PhyPayload::Data(d) => match d {
                DataPayload::Encrypted(data) => {
                    defmt::write!(f, "DataPayload::Encrypted({})", data.0);
                }
                DataPayload::Decrypted(data) => {
                    defmt::write!(f, "DataPayload::Decrypted({})", data.0);
                }
            },
        };
    }
}

impl<T: AsRef<[u8]>, F> AsRef<[u8]> for PhyPayload<T, F> {
    fn as_ref(&self) -> &[u8] {
        match self {
            PhyPayload::JoinRequest(jr) => jr.as_bytes(),
            PhyPayload::JoinAccept(ja) => ja.as_bytes(),
            PhyPayload::Data(data) => data.as_bytes(),
        }
    }
}

/// JoinAcceptPayload is a type that represents a JoinAccept.
///
/// It can either be encrypted for example as a result from the [parse](fn.parse.html)
/// function or not.
#[derive(Debug, PartialEq, Eq)]
pub enum JoinAcceptPayload<T, F> {
    Encrypted(EncryptedJoinAcceptPayload<T, F>),
    Decrypted(DecryptedJoinAcceptPayload<T, F>),
}

impl<T: AsRef<[u8]>, F> AsPhyPayloadBytes for JoinAcceptPayload<T, F> {
    fn as_bytes(&self) -> &[u8] {
        match self {
            JoinAcceptPayload::Encrypted(e) => e.as_bytes(),
            JoinAcceptPayload::Decrypted(d) => d.as_bytes(),
        }
    }
}

/// DataPayload is a type that represents a ConfirmedDataUp, ConfirmedDataDown,
/// UnconfirmedDataUp or UnconfirmedDataDown.
///
/// It can either be encrypted for example as a result from the [parse](fn.parse.html)
/// function or not.
#[derive(Debug, PartialEq, Eq)]
pub enum DataPayload<T, F> {
    Encrypted(EncryptedDataPayload<T, F>),
    Decrypted(DecryptedDataPayload<T>),
}

impl<T: AsRef<[u8]>, F> DataHeader for DataPayload<T, F> {
    fn as_data_bytes(&self) -> &[u8] {
        match self {
            DataPayload::Encrypted(data) => data.as_data_bytes(),
            DataPayload::Decrypted(data) => data.as_data_bytes(),
        }
    }
}

/// Trait with the sole purpose to make clear distinction in some implementations between types
/// that just happen to have AsRef and those that want to have the given implementations (like
/// MICAble and MHDRAble).
pub trait AsPhyPayloadBytes {
    fn as_bytes(&self) -> &[u8];
}

impl AsRef<[u8]> for dyn AsPhyPayloadBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

/// Helper trait to add mic to all types that should have it.
pub trait MICAble {
    /// Gives the MIC of the PhyPayload.
    fn mic(&self) -> MIC;
}

impl<T: AsPhyPayloadBytes> MICAble for T {
    fn mic(&self) -> MIC {
        let data = self.as_bytes();
        let len = data.len();
        MIC([data[len - 4], data[len - 3], data[len - 2], data[len - 1]])
    }
}

/// Helper trait to add mhdr to all types that should have it.
pub trait MHDRAble {
    /// Gives the MIC of the PhyPayload.
    fn mhdr(&self) -> MHDR;
}

/// Assumes at least one byte in the data.
impl<T: AsPhyPayloadBytes> MHDRAble for T {
    fn mhdr(&self) -> MHDR {
        let data = self.as_bytes();
        MHDR(data[0])
    }
}

/// JoinAcceptPayload represents a JoinRequest.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [parse](fn.parse.html) function.
#[derive(Debug, PartialEq, Eq)]
pub struct JoinRequestPayload<T, F>(T, F);

impl<T: AsRef<[u8]>, F> AsPhyPayloadBytes for JoinRequestPayload<T, F> {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>, F: CryptoFactory> JoinRequestPayload<T, F> {
    /// Creates a new JoinRequestPayload if the provided data is acceptable.
    ///
    /// # Argument
    ///
    /// * data - the bytes for the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// let data = vec![0x00, 0x04, 0x03, 0x02, 0x01, 0x04, 0x03, 0x02, 0x01, 0x05, 0x04, 0x03,
    ///     0x02, 0x05, 0x04, 0x03, 0x02, 0x2d, 0x10, 0x6a, 0x99, 0x0e, 0x12];
    /// let phy = lorawan::parser::JoinRequestPayload::new_with_factory(data,
    ///     lorawan::default_crypto::DefaultFactory);
    /// ```
    pub fn new_with_factory(data: T, factory: F) -> Result<Self, Error> {
        if !Self::can_build_from(data.as_ref()) {
            Err(Error::InvalidData)
        } else {
            Ok(Self(data, factory))
        }
    }

    fn can_build_from(bytes: &[u8]) -> bool {
        bytes.len() == JOIN_REQUEST_LEN && MHDR(bytes[0]).mtype() == MType::JoinRequest
    }

    /// Gives the APP EUI of the JoinRequest.
    pub fn app_eui(&self) -> EUI64<&[u8]> {
        EUI64::new_from_raw(&self.0.as_ref()[1..9])
    }

    /// Gives the DEV EUI of the JoinRequest.
    pub fn dev_eui(&self) -> EUI64<&[u8]> {
        EUI64::new_from_raw(&self.0.as_ref()[9..17])
    }

    /// Gives the DEV Nonce of the JoinRequest.
    pub fn dev_nonce(&self) -> DevNonce<&[u8]> {
        DevNonce::new_from_raw(&self.0.as_ref()[17..19])
    }

    /// Verifies that the JoinRequest has correct MIC.
    pub fn validate_mic(&self, key: &AES128) -> bool {
        self.mic() == self.calculate_mic(key)
    }

    fn calculate_mic(&self, key: &AES128) -> MIC {
        let d = self.0.as_ref();
        securityhelpers::calculate_mic(&d[..d.len() - MIC_LEN], self.1.new_mac(key))
    }
}

/// EncryptedJoinAcceptPayload represents an encrypted JoinAccept.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [parse](fn.parse.html) function.
#[derive(Debug, PartialEq, Eq)]
pub struct EncryptedJoinAcceptPayload<T, F>(T, F);

impl<T: AsRef<[u8]>, F> AsPhyPayloadBytes for EncryptedJoinAcceptPayload<T, F> {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>, F: CryptoFactory> EncryptedJoinAcceptPayload<T, F> {
    /// Creates a new EncryptedJoinAcceptPayload if the provided data is acceptable.
    ///
    /// # Argument
    ///
    /// * data - the bytes for the payload.
    /// * factory - the factory that shall be used to create object for crypto functions.
    pub fn new_with_factory(data: T, factory: F) -> Result<Self, Error> {
        if Self::can_build_from(data.as_ref()) {
            Ok(Self(data, factory))
        } else {
            Err(Error::InvalidData)
        }
    }

    fn can_build_from(bytes: &[u8]) -> bool {
        (bytes.len() == JOIN_ACCEPT_LEN || bytes.len() == JOIN_ACCEPT_WITH_CFLIST_LEN)
            && MHDR(bytes[0]).mtype() == MType::JoinAccept
    }

    /// Decrypts the EncryptedJoinAcceptPayload producing a DecryptedJoinAcceptPayload.
    ///
    /// This method consumes the EncryptedJoinAcceptPayload as it reuses the underlying memory.
    /// Please note that it does not verify the mic.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for the decryption.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut data = vec![0x20, 0x49, 0x3e, 0xeb, 0x51, 0xfb, 0xa2, 0x11, 0x6f, 0x81, 0x0e, 0xdb,
    ///     0x37, 0x42, 0x97, 0x51, 0x42];
    /// let phy = lorawan::parser::EncryptedJoinAcceptPayload::new(data);
    /// let key = lorawan::keys::AppKey::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ///     0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let decrypted = phy.unwrap().decrypt(&key);
    /// ```
    pub fn decrypt(mut self, key: &AppKey) -> DecryptedJoinAcceptPayload<T, F> {
        {
            let bytes = self.0.as_mut();
            let len = bytes.len();
            let aes_enc = self.1.new_enc(&key.0);

            for i in 0..(len >> 4) {
                let start = (i << 4) + 1;
                let block = GenericArray::from_mut_slice(&mut bytes[start..(start + 16)]);
                aes_enc.encrypt_block(block);
            }
        }
        DecryptedJoinAcceptPayload(self.0, self.1)
    }
}

/// DecryptedJoinAcceptPayload represents a decrypted JoinAccept.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [EncryptedJoinAcceptPayload.decrypt](struct.EncryptedJoinAcceptPayload.html#method.decrypt)
/// function.
#[derive(Debug, PartialEq, Eq)]
pub struct DecryptedJoinAcceptPayload<T, F>(T, F);

impl<T: AsRef<[u8]>, F> AsPhyPayloadBytes for DecryptedJoinAcceptPayload<T, F> {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>, F: CryptoFactory> DecryptedJoinAcceptPayload<T, F> {
    /// Verifies that the JoinAccept has correct MIC.
    pub fn validate_mic(&self, key: &AppKey) -> bool {
        self.mic() == self.calculate_mic(key)
    }

    pub fn calculate_mic(&self, key: &AppKey) -> MIC {
        let d = self.0.as_ref();
        securityhelpers::calculate_mic(&d[..d.len() - MIC_LEN], self.1.new_mac(&key.0))
    }

    /// Computes the network session key for a given device.
    ///
    /// # Argument
    ///
    /// * app_nonce - the network server nonce.
    /// * nwk_addr - the address of the network.
    /// * dev_nonce - the nonce from the device.
    /// * key - the app key.
    ///
    /// # Examples
    ///
    /// ```
    /// let dev_nonce = vec![0xcc, 0xdd];
    /// let data = vec![0x20, 0x49, 0x3e, 0xeb, 0x51, 0xfb, 0xa2, 0x11, 0x6f, 0x81, 0x0e, 0xdb, 0x37,
    ///     0x42, 0x97, 0x51, 0x42];
    /// let app_key = lorawan::keys::AppKey::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ///     0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let join_accept = lorawan::parser::DecryptedJoinAcceptPayload::new(data, &app_key).unwrap();
    ///
    /// let nwk_skey = join_accept.derive_newskey(
    ///     &lorawan::parser::DevNonce::new(&dev_nonce[..]).unwrap(),
    ///     &app_key,
    /// );
    /// ```
    pub fn derive_newskey<TT: AsRef<[u8]>>(
        &self,
        dev_nonce: &DevNonce<TT>,
        key: &AppKey,
    ) -> NewSKey {
        NewSKey(self.derive_session_key(0x1, dev_nonce, &key.0))
    }

    /// Computes the application session key for a given device.
    ///
    /// # Argument
    ///
    /// * app_nonce - the network server nonce.
    /// * nwk_addr - the address of the network.
    /// * dev_nonce - the nonce from the device.
    /// * key - the app key.
    ///
    /// # Examples
    ///
    /// ```
    /// let dev_nonce = vec![0xcc, 0xdd];
    /// let data = vec![0x20, 0x49, 0x3e, 0xeb, 0x51, 0xfb, 0xa2, 0x11, 0x6f, 0x81, 0x0e, 0xdb, 0x37,
    ///     0x42, 0x97, 0x51, 0x42];
    /// let app_key = lorawan::keys::AppKey::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ///     0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let join_accept = lorawan::parser::DecryptedJoinAcceptPayload::new(data, &app_key).unwrap();
    ///
    /// let app_skey = join_accept.derive_appskey(
    ///     &lorawan::parser::DevNonce::new(&dev_nonce[..]).unwrap(),
    ///     &app_key,
    /// );
    /// ```
    pub fn derive_appskey<TT: AsRef<[u8]>>(
        &self,
        dev_nonce: &DevNonce<TT>,
        key: &AppKey,
    ) -> AppSKey {
        AppSKey(self.derive_session_key(0x2, dev_nonce, &key.0))
    }

    fn derive_session_key<TT: AsRef<[u8]>>(
        &self,
        first_byte: u8,
        dev_nonce: &DevNonce<TT>,
        key: &AES128,
    ) -> AES128 {
        let cipher = self.1.new_enc(key);

        // note: AppNonce is 24 bits, NetId is 24 bits, DevNonce is 16 bits
        let app_nonce = self.app_nonce();
        let nwk_addr = self.net_id();
        let (app_nonce_arr, nwk_addr_arr, dev_nonce_arr) =
            (app_nonce.as_ref(), nwk_addr.as_ref(), dev_nonce.as_ref());

        let mut block = [0u8; 16];
        block[0] = first_byte;
        block[1] = app_nonce_arr[0];
        block[2] = app_nonce_arr[1];
        block[3] = app_nonce_arr[2];
        block[4] = nwk_addr_arr[0];
        block[5] = nwk_addr_arr[1];
        block[6] = nwk_addr_arr[2];
        block[7] = dev_nonce_arr[0];
        block[8] = dev_nonce_arr[1];

        let mut input = GenericArray::clone_from_slice(&block);
        cipher.encrypt_block(&mut input);

        let mut output_key = [0u8; 16];
        output_key.copy_from_slice(&input[0..16]);
        AES128(output_key)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CfList<'a> {
    DynamicChannel([Frequency<'a>; 5]),
    FixedChannel(ChannelMask<9>),
}

impl<T: AsRef<[u8]>, F> DecryptedJoinAcceptPayload<T, F> {
    /// Gives the app nonce of the JoinAccept.
    pub fn app_nonce(&self) -> AppNonce<&[u8]> {
        const OFFSET: usize = MHDR_LEN;
        const END: usize = OFFSET + JOIN_NONCE_LEN;
        AppNonce::new_from_raw(&self.0.as_ref()[OFFSET..END])
    }

    /// Gives the net ID of the JoinAccept.
    pub fn net_id(&self) -> NwkAddr<&[u8]> {
        const OFFSET: usize = MHDR_LEN + JOIN_NONCE_LEN;
        const END: usize = OFFSET + NET_ID_LEN;
        NwkAddr::new_from_raw(&self.0.as_ref()[OFFSET..END])
    }

    /// Gives the dev address of the JoinAccept.
    pub fn dev_addr(&self) -> DevAddr<&[u8]> {
        const OFFSET: usize = MHDR_LEN + JOIN_NONCE_LEN + NET_ID_LEN;
        const END: usize = OFFSET + DEV_ADDR_LEN;
        DevAddr::new_from_raw(&self.0.as_ref()[OFFSET..END])
    }

    /// Gives the downlink configuration of the JoinAccept.
    pub fn dl_settings(&self) -> DLSettings {
        const OFFSET: usize = MHDR_LEN + JOIN_NONCE_LEN + NET_ID_LEN + DEV_ADDR_LEN;
        DLSettings::new(self.0.as_ref()[OFFSET])
    }

    /// Gives the RX delay of the JoinAccept.
    pub fn rx_delay(&self) -> u8 {
        const OFFSET: usize =
            MHDR_LEN + JOIN_NONCE_LEN + NET_ID_LEN + DEV_ADDR_LEN + DL_SETTINGS_LEN;
        self.0.as_ref()[OFFSET] & 0x0f
    }

    /// Gives the channel frequency list of the JoinAccept.
    pub fn c_f_list(&self) -> Option<CfList<'_>> {
        if self.0.as_ref().len() == JOIN_ACCEPT_LEN {
            return None;
        }

        let d = self.0.as_ref();
        let c_f_list_type = d[28];
        if c_f_list_type == 0 {
            let res = [
                Frequency::new_from_raw(&d[13..16]),
                Frequency::new_from_raw(&d[16..19]),
                Frequency::new_from_raw(&d[19..22]),
                Frequency::new_from_raw(&d[22..25]),
                Frequency::new_from_raw(&d[25..28]),
            ];
            Some(CfList::DynamicChannel(res))
        } else if c_f_list_type == 1 {
            Some(CfList::FixedChannel(ChannelMask::new_from_raw(&d[13..22])))
        } else {
            None
        }
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>, F: CryptoFactory> DecryptedJoinAcceptPayload<T, F> {
    /// Creates a DecryptedJoinAcceptPayload from the bytes of a JoinAccept.
    ///
    /// The JoinAccept payload is automatically decrypted and the mic is verified using the suplied
    /// crypto factory implementation.
    ///
    /// # Argument
    ///
    /// * bytes - the data from which the PhyPayload is to be built.
    /// * key - the key that is to be used to decrypt the payload.
    /// * factory - the factory that shall be used to create object for crypto functions.
    pub fn new_with_factory(data: T, key: &AppKey, factory: F) -> Result<Self, Error> {
        let t = EncryptedJoinAcceptPayload::new_with_factory(data, factory)?;
        let res = t.decrypt(key);
        if res.validate_mic(key) {
            Ok(res)
        } else {
            Err(Error::InvalidMic)
        }
    }
}

/// Helper trait for EncryptedDataPayload and DecryptedDataPayload.
///
/// NOTE: Does not check the payload size as that should be done prior to building the object of
/// the implementing type.
pub trait DataHeader {
    /// Equivalent to AsRef<[u8]>.
    fn as_data_bytes(&self) -> &[u8];

    /// Gives the FHDR of the DataPayload.
    fn fhdr(&self) -> FHDR<'_> {
        FHDR::new_from_raw(&self.as_data_bytes()[1..(1 + self.fhdr_length())], self.is_uplink())
    }

    /// Gives whether the frame is confirmed
    fn is_confirmed(&self) -> bool {
        let mtype = MHDR(self.as_data_bytes()[0]).mtype();
        mtype == MType::ConfirmedDataUp || mtype == MType::ConfirmedDataDown
    }

    /// Gives whether the payload is uplink or not.
    fn is_uplink(&self) -> bool {
        let mtype = MHDR(self.as_data_bytes()[0]).mtype();
        mtype == MType::UnconfirmedDataUp || mtype == MType::ConfirmedDataUp
    }

    /// Gives the FPort of the DataPayload if there is one.
    fn f_port(&self) -> Option<u8> {
        let fhdr_length = self.fhdr_length();
        let data = self.as_data_bytes();
        if fhdr_length + 1 >= data.len() - 5 {
            return None;
        }
        Some(data[1 + fhdr_length])
    }

    /// Gives the length of the FHDR field.
    fn fhdr_length(&self) -> usize {
        fhdr_length(self.as_data_bytes()[5])
    }
}

fn fhdr_length(b: u8) -> usize {
    7 + (b & 0x0f) as usize
}

impl<T: DataHeader> AsPhyPayloadBytes for T {
    fn as_bytes(&self) -> &[u8] {
        self.as_data_bytes()
    }
}

/// EncryptedDataPayload represents an encrypted data payload.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [parse](fn.parse.html) function.
#[derive(Debug, PartialEq, Eq)]
pub struct EncryptedDataPayload<T, F>(T, F);

impl<T: AsRef<[u8]>, F> DataHeader for EncryptedDataPayload<T, F> {
    fn as_data_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>, F: CryptoFactory> EncryptedDataPayload<T, F> {
    /// Creates a new EncryptedDataPayload if the provided data is acceptable.
    ///
    /// # Argument
    ///
    /// * data - the bytes for the payload.
    /// * factory - the factory that shall be used to create object for crypto functions.
    pub fn new_with_factory(data: T, factory: F) -> Result<Self, Error> {
        if Self::can_build_from(data.as_ref()) {
            Ok(Self(data, factory))
        } else {
            Err(Error::InvalidData)
        }
    }

    fn can_build_from(bytes: &[u8]) -> bool {
        // The smallest packets contain MHDR (1 bytes) + FHDR + MIC (4 bytes).
        if bytes.len() < 12 || 5 + fhdr_length(bytes[5]) > bytes.len() {
            return false;
        }

        matches!(
            MHDR(bytes[0]).mtype(),
            MType::ConfirmedDataUp
                | MType::ConfirmedDataDown
                | MType::UnconfirmedDataUp
                | MType::UnconfirmedDataDown
        )
    }

    /// Verifies that the DataPayload has correct MIC.
    pub fn validate_mic(&self, key: &AES128, fcnt: u32) -> bool {
        self.mic() == self.calculate_mic(key, fcnt)
    }

    fn calculate_mic(&self, key: &AES128, fcnt: u32) -> MIC {
        let d = self.0.as_ref();
        securityhelpers::calculate_data_mic(&d[..d.len() - MIC_LEN], self.1.new_mac(key), fcnt)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>, F: CryptoFactory> EncryptedDataPayload<T, F> {
    /// Decrypts the EncryptedDataPayload payload.
    ///
    /// This method consumes the EncryptedDataPayload as it reuses the underlying memory. Please
    /// note that it does not verify the mic.
    ///
    /// If used on the application server side for application payload decryption, the nwk_skey can
    /// be None. If used on the network server side and the app_skey is not available, app_skey can
    /// be None when fport is 0. Failure to meet those constraints will result in an Err being
    /// returned.
    ///
    /// # Argument
    ///
    /// * nwk_skey - the Network Session key used to decrypt the mac commands in case the payload is
    ///   transporting those.
    /// * app_skey - the Application Session key used to decrypt the application payload in case the
    ///   payload is transporting that.
    /// * fcnt - the counter used to encrypt the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01,
    ///     0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6, 0xc3, 0xb5, 0x82];
    /// let key = lorawan::keys::AES128([1; 16]);
    /// let enc_phy = lorawan::parser::EncryptedDataPayload::new(data).unwrap();
    /// let dec_phy = enc_phy.decrypt(None, Some(&key), 1);
    /// ```
    pub fn decrypt<'a>(
        mut self,
        nwk_skey: Option<&'a AES128>,
        app_skey: Option<&'a AES128>,
        fcnt: u32,
    ) -> Result<DecryptedDataPayload<T>, Error> {
        let fhdr_length = self.fhdr_length();
        let fhdr = self.fhdr();
        let full_fcnt = compute_fcnt(fcnt, fhdr.fcnt());
        let key = if self.f_port().is_some() && self.f_port().unwrap() != 0 {
            app_skey
        } else {
            nwk_skey
        };
        if key.is_none() {
            return Err(Error::InvalidKey);
        }
        let data = self.0.as_mut();
        let len = data.len();
        let start = MHDR_LEN + FPORT_LEN + fhdr_length;
        let end = len - MIC_LEN;
        if start < end {
            securityhelpers::encrypt_frm_data_payload(
                data,
                start,
                end,
                full_fcnt,
                &self.1.new_enc(key.unwrap()),
            );
        }

        Ok(DecryptedDataPayload(self.0))
    }

    /// Verifies the mic and decrypts the EncryptedDataPayload payload if mic matches.
    ///
    /// This is helper method that combines validate_mic and decrypt. In case the mic is fine, it
    /// consumes the EncryptedDataPayload and reuses the underlying memory to produce
    /// DecryptedDataPayload. If the mic does not match, it returns the original
    /// EncryptedDataPayload so that it can be tried against the keys of another device that shares
    /// the same dev_addr. For an example please see [decrypt](#method.decrypt).
    pub fn decrypt_if_mic_ok<'a>(
        self,
        nwk_skey: &'a AES128,
        app_skey: &'a AES128,
        fcnt: u32,
    ) -> Result<DecryptedDataPayload<T>, Self> {
        if !self.validate_mic(nwk_skey, fcnt) {
            Err(self)
        } else {
            Ok(self.decrypt(Some(nwk_skey), Some(app_skey), fcnt).unwrap())
        }
    }
}

fn compute_fcnt(old_fcnt: u32, fcnt: u16) -> u32 {
    ((old_fcnt >> 16) << 16) ^ u32::from(fcnt)
}

/// DecryptedDataPayload represents a decrypted DataPayload.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [EncryptedDataPayload.decrypt](struct.EncryptedDataPayload.html#method.decrypt) function.
#[derive(Debug, PartialEq, Eq)]
pub struct DecryptedDataPayload<T>(T);

impl<T> DecryptedDataPayload<T> {
    pub fn to_inner(self) -> T {
        self.0
    }
}

#[cfg(feature = "defmt")]
impl<T: AsRef<[u8]>> defmt::Format for DecryptedDataPayload<T> {
    fn format(&self, fmt: defmt::Formatter<'_>) {
        defmt::write!(fmt, "DecryptedDataPayload {{ {:?} }}", self.0.as_ref())
    }
}

impl<T: AsRef<[u8]>> DataHeader for DecryptedDataPayload<T> {
    fn as_data_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> DecryptedDataPayload<T> {
    /// Returns FRMPayload that can represent either application payload or mac commands if fport
    /// is 0.
    pub fn frm_payload(&self) -> FRMPayload<'_> {
        let data = self.as_data_bytes();
        let len = data.len();
        let fhdr_length = self.fhdr_length();
        //we have more bytes than fhdr + fport
        if len < fhdr_length + 6 {
            FRMPayload::None
        } else if self.f_port() != Some(0) {
            // the size guarantees the existance of f_port
            FRMPayload::Data(&data[(1 + fhdr_length + 1)..(len - 4)])
        } else {
            FRMPayload::MACCommands(FRMMacCommands::new(
                &data[(1 + fhdr_length + 1)..(len - 4)],
                self.is_uplink(),
            ))
        }
    }
}

/// Parses a payload as LoRaWAN physical payload.
///
/// # Argument
///
/// * bytes - the data from which the PhyPayload is to be built.
///
/// # Examples
///
/// ```
/// let mut data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01,
///     0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6, 0xc3, 0xb5, 0x82];
/// if let Ok(lorawan::parser::PhyPayload::Data(phy)) = lorawan::parser::parse(data) {
///     println!("{:?}", phy);
/// } else {
///     panic!("failed to parse data payload");
/// }
/// ```
#[cfg(feature = "default-crypto")]
pub fn parse<T: AsRef<[u8]> + AsMut<[u8]>>(
    data: T,
) -> Result<PhyPayload<T, DefaultFactory>, Error> {
    parse_with_factory(data, DefaultFactory)
}

/// Parses a payload as LoRaWAN physical payload.
///
/// Check out [parse](fn.parse.html) if you do not need custom crypto factory.
///
/// Returns error "Unsupported major version" if the major version is unsupported.
///
/// # Argument
///
/// * bytes - the data from which the PhyPayload is to be built.
/// * factory - the factory that shall be used to create object for crypto functions.
pub fn parse_with_factory<T, F>(data: T, factory: F) -> Result<PhyPayload<T, F>, Error>
where
    T: AsRef<[u8]> + AsMut<[u8]>,
    F: CryptoFactory,
{
    let bytes = data.as_ref();
    // Enough data for payload (bare minimum without fport and FRMPayload)?
    if bytes.len() < PHY_PAYLOAD_MIN_LEN {
        return Err(Error::InvalidPayload);
    }
    let mhdr = MHDR(bytes[0]);
    if mhdr.major() != Major::LoRaWANR1 {
        return Err(Error::UnsupportedMajorVersion);
    }
    match mhdr.mtype() {
        MType::JoinRequest => {
            Ok(PhyPayload::JoinRequest(JoinRequestPayload::new_with_factory(data, factory)?))
        }
        MType::JoinAccept => Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(
            EncryptedJoinAcceptPayload::new_with_factory(data, factory)?,
        ))),
        MType::UnconfirmedDataUp
        | MType::ConfirmedDataUp
        | MType::UnconfirmedDataDown
        | MType::ConfirmedDataDown => Ok(PhyPayload::Data(DataPayload::Encrypted(
            EncryptedDataPayload::new_with_factory(data, factory)?,
        ))),
        _ => Err(Error::InvalidMessageType),
    }
}

/// MHDR represents LoRaWAN MHDR.
#[derive(Debug, PartialEq, Eq)]
pub struct MHDR(u8);

impl MHDR {
    pub fn new(byte: u8) -> MHDR {
        MHDR(byte)
    }

    /// Type of message PhyPayload is carrying.
    pub fn mtype(&self) -> MType {
        match self.0 >> 5 {
            0 => MType::JoinRequest,
            1 => MType::JoinAccept,
            2 => MType::UnconfirmedDataUp,
            3 => MType::UnconfirmedDataDown,
            4 => MType::ConfirmedDataUp,
            5 => MType::ConfirmedDataDown,
            6 => MType::RFU,
            _ => MType::Proprietary,
        }
    }

    /// Version of LoRaWAN payload format.
    pub fn major(&self) -> Major {
        if self.0.trailing_zeros() >= 2 {
            Major::LoRaWANR1
        } else {
            Major::RFU
        }
    }
}

impl From<u8> for MHDR {
    fn from(v: u8) -> Self {
        MHDR(v)
    }
}

/// MType gives the possible message types of the PhyPayload.
#[derive(Debug, PartialEq, Eq)]
pub enum MType {
    JoinRequest,
    JoinAccept,
    UnconfirmedDataUp,
    UnconfirmedDataDown,
    ConfirmedDataUp,
    ConfirmedDataDown,
    RFU,
    Proprietary,
}

/// Major gives the supported LoRaWAN payload formats.
#[derive(Debug, PartialEq, Eq)]
pub enum Major {
    LoRaWANR1,
    RFU,
}

fixed_len_struct! {
    /// EUI64 represents a 64-bit EUI.
    struct EUI64[8];
}

fixed_len_struct! {
    /// DevNonce represents a 16-bit device nonce.
    struct DevNonce[2];
}

impl From<DevNonce<[u8; 2]>> for u16 {
    fn from(v: DevNonce<[u8; 2]>) -> Self {
        u16::from_be_bytes(v.0)
    }
}

impl From<u16> for DevNonce<[u8; 2]> {
    fn from(v: u16) -> Self {
        Self::from(v.to_be_bytes())
    }
}

fixed_len_struct! {
    /// AppNonce represents a 24-bit network server nonce.
    struct AppNonce[3];
}

fixed_len_struct! {
    /// DevAddr represents a 32-bit device address.
    struct DevAddr[4];
}

#[allow(clippy::should_implement_trait)]
impl<T: AsRef<[u8]>> DevAddr<T> {
    pub fn nwk_id(&self) -> u8 {
        self.0.as_ref()[0] >> 1
    }
    pub fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<DevAddr<[u8; 4]>> for u32 {
    fn from(v: DevAddr<[u8; 4]>) -> Self {
        u32::from_be_bytes(v.0)
    }
}

impl From<u32> for DevAddr<[u8; 4]> {
    fn from(v: u32) -> Self {
        Self::from(v.to_be_bytes())
    }
}

fixed_len_struct! {
    /// NwkAddr represents a 24-bit network address.
    struct NwkAddr[3];
}

/// FHDR represents FHDR from DataPayload.
#[derive(Debug, PartialEq, Eq)]
pub struct FHDR<'a>(&'a [u8], bool);

impl<'a> FHDR<'a> {
    pub fn new_from_raw(bytes: &'a [u8], uplink: bool) -> FHDR<'_> {
        FHDR(bytes, uplink)
    }

    pub fn new(bytes: &'a [u8], uplink: bool) -> Option<FHDR<'_>> {
        let data_len = bytes.len();
        if data_len < 7 {
            return None;
        }
        if data_len < fhdr_length(bytes[4]) {
            return None;
        }
        Some(FHDR(bytes, uplink))
    }

    /// Gives the device address associated with the given payload.
    pub fn dev_addr(&self) -> DevAddr<&'a [u8]> {
        DevAddr::new_from_raw(&self.0[0..4])
    }

    /// Gives the FCtrl associated with the given payload.
    pub fn fctrl(&self) -> FCtrl {
        FCtrl(self.0[4], self.1)
    }

    /// Gives the truncated FCnt associated with the given payload.
    pub fn fcnt(&self) -> u16 {
        (u16::from(self.0[6]) << 8) | u16::from(self.0[5])
    }

    /// Gives the size of FOpts.
    pub fn fopts_len(&self) -> u8 {
        FCtrl(self.0[4], self.1).f_opts_len()
    }

    pub fn data(&self) -> &[u8] {
        &self.0[7_usize..(7 + self.fopts_len()) as usize]
    }
}

/// FCtrl represents the FCtrl from FHDR.
#[derive(Debug, PartialEq, Eq)]
pub struct FCtrl(pub u8, pub bool);

impl FCtrl {
    pub fn set_ack(&mut self) {
        self.0 |= 1 << 5;
    }

    pub fn new(bytes: u8, uplink: bool) -> FCtrl {
        FCtrl(bytes, uplink)
    }

    /// Set ADR ACK requested.
    pub fn set_adr_ack_req(&mut self) {
        self.0 |= 1 << 6;
    }

    /// Set ADR enabled.
    pub fn set_adr(&mut self) {
        self.0 |= 1 << 7;
    }

    /// Gives whether ADR is enabled or not.
    pub fn adr(&self) -> bool {
        self.0 >> 7 == 1
    }

    /// Gives whether ADR ACK is requested.
    pub fn adr_ack_req(&self) -> bool {
        self.1 && self.0 & (1 << 6) != 0
    }

    /// Gives whether ack bit is set.
    pub fn ack(&self) -> bool {
        self.0 & (1 << 5) != 0
    }

    /// Gives whether there are more payloads pending.
    pub fn f_pending(&self) -> bool {
        !self.1 && self.0 & (1 << 4) != 0
    }

    /// Gives the size of FOpts.
    pub fn f_opts_len(&self) -> u8 {
        self.0 & 0x0f
    }

    /// Gives the binary representation of the FCtrl.
    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

/// FRMPayload represents the FRMPayload that can either be the application
/// data or mac commands.
#[derive(Debug, PartialEq, Eq)]
pub enum FRMPayload<'a> {
    Data(&'a [u8]),
    MACCommands(FRMMacCommands<'a>),
    None,
}

/// FRMMacCommands represents the mac commands.
#[derive(Debug, PartialEq, Eq)]
pub struct FRMMacCommands<'a>(bool, &'a [u8]);

impl<'a> FRMMacCommands<'a> {
    pub fn new(bytes: &'a [u8], uplink: bool) -> Self {
        FRMMacCommands(uplink, bytes)
    }

    pub fn data(&self) -> &[u8] {
        self.1
    }

    // /// Gives the list of mac commands represented in the FRMPayload.
    // pub fn mac_commands(&self) -> MacCommandIterator {
    //     parse_mac_commands(self.1, self.0)
    // }
}
