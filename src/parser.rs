// Copyright (c) 2017,2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

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
//!     if let Ok(FRMPayload::Data(data_payload)) =
//!             decrypted.frm_payload() {
//!         println!("{}", String::from_utf8_lossy(data_payload));
//!     }
//! } else {
//!     panic!("failed to parse data payload");
//! }
//! ```

use aes::block_cipher_trait::generic_array;
use aes::block_cipher_trait::BlockCipher;
use aes::Aes128;

use heapless;
use heapless::consts::*;

type Vec<T> = heapless::Vec<T,U256>;

use super::keys;
use super::maccommands;
use super::securityhelpers;

macro_rules! fixed_len_struct {
    (
        $(#[$outer:meta])*
        struct $type:ident[$size:expr];
    ) => {
        $(#[$outer])*
        #[derive(Debug, Eq)]
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

        impl<T: AsRef<[u8]>> AsRef<[u8]> for $type<T> {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }
    };
}

/// PhyPayload is a type that represents a physical LoRaWAN payload.
///
/// It can either be JoinRequest, JoinAccept, or DataPayload.
#[derive(Debug, PartialEq)]
pub enum PhyPayload<T: AsRef<[u8]> +  AsMut<[u8]>> {
    JoinRequest(JoinRequestPayload<T>),
    JoinAccept(JoinAcceptPayload<T>),
    Data(DataPayload<T>)
}

/// JoinAcceptPayload is a type that represents a JoinAccept.
///
/// It can either be encrypted for example as a result from the [parse](fn.parse.html)
/// function or not.
#[derive(Debug, PartialEq)]
pub enum JoinAcceptPayload<T: AsRef<[u8]> + AsMut<[u8]>> {
    Encrypted(EncryptedJoinAcceptPayload<T>),
    Decrypted(DecryptedJoinAcceptPayload<T>)
}

/// DataPayload is a type that represents a ConfirmedDataUp, ConfirmedDataDown,
/// UnconfirmedDataUp or UnconfirmedDataDown.
///
/// It can either be encrypted for example as a result from the [parse](fn.parse.html)
/// function or not.
#[derive(Debug, PartialEq)]
pub enum DataPayload<T: AsRef<[u8]> + AsMut<[u8]>> {
    Encrypted(EncryptedDataPayload<T>),
    Decrypted(DecryptedDataPayload<T>)
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
    fn mic(&self) -> keys::MIC;

}

impl<T: AsPhyPayloadBytes> MICAble for T {
    fn mic(&self) -> keys::MIC {
        let data = self.as_bytes();
        let len = data.len();
        keys::MIC([data[len - 4], data[len - 3], data[len - 2], data[len - 1]])
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
#[derive(Debug, PartialEq)]
pub struct JoinRequestPayload<T: AsRef<[u8]>>(T);

impl<T: AsRef<[u8]>> AsPhyPayloadBytes for JoinRequestPayload<T> {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> JoinRequestPayload<T> {
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
    /// let phy = lorawan::parser::JoinRequestPayload::new(data);
    /// ```
    pub fn new<'a>(data: T) -> Result<Self, &'a str> {
        if !JoinRequestPayload::<T>::can_build_from(data.as_ref()) {
            Err("can not build JoinRequestPayload from the provided data")
        } else {
            Ok(Self(data))
        }
    }

    fn can_build_from(bytes: &[u8]) -> bool {
        bytes.len() == 23 && MHDR(bytes[0]).mtype() == MType::JoinRequest
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
    pub fn validate_mic(&self, key: &keys::AES128) -> bool {
        self.mic() == self.calculate_mic(key)
    }

    fn calculate_mic(&self, key: &keys::AES128) -> keys::MIC {
        let d = self.0.as_ref();
        securityhelpers::calculate_mic(&d[..d.len() - 4], key)
    }
}

/// EncryptedJoinAcceptPayload represents an encrypted JoinAccept.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [parse](fn.parse.html) function.
#[derive(Debug, PartialEq)]
pub struct EncryptedJoinAcceptPayload<T: AsRef<[u8]>>(T);

impl<T: AsRef<[u8]>> AsPhyPayloadBytes for EncryptedJoinAcceptPayload<T> {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> EncryptedJoinAcceptPayload<T> {
    /// Creates a new EncryptedJoinAcceptPayload if the provided data is acceptable.
    ///
    /// # Argument
    ///
    /// * data - the bytes for the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// let data = vec![0x20, 0x49, 0x3e, 0xeb, 0x51, 0xfb, 0xa2, 0x11, 0x6f, 0x81, 0x0e, 0xdb,
    ///     0x37, 0x42, 0x97, 0x51, 0x42];
    /// let phy = lorawan::parser::EncryptedJoinAcceptPayload::new(data);
    /// ```
    pub fn new<'a>(data: T) -> Result<Self, &'a str> {
        if EncryptedJoinAcceptPayload::<T>::can_build_from(data.as_ref()) {
            Ok(Self(data))
        } else {
            Err("can not build EncryptedJoinAcceptPayload from the provided data")
        }
    }

    fn can_build_from(bytes: &[u8]) -> bool {
        (bytes.len() == 17 || bytes.len() == 33) && MHDR(bytes[0]).mtype() == MType::JoinAccept
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
    /// let key = lorawan::keys::AES128([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ///     0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let decrypted = phy.unwrap().decrypt(&key);
    /// ```
    pub fn decrypt(mut self, key: &keys::AES128) -> DecryptedJoinAcceptPayload<T> {
        {
            let bytes = self.0.as_mut();
            let len = bytes.len();
            let k = generic_array::GenericArray::from_slice(&key.0[..]);
            let aes_enc = Aes128::new(k);

            for i in 0..(len >> 4) {
                let start = (i << 4) + 1;
                // TODO: try to remove the copying
                let mut block = generic_array::GenericArray::clone_from_slice(&bytes[start..(start + 16)]);
                aes_enc.encrypt_block(&mut block);
                bytes[start..(16+start)].clone_from_slice(&block[..16])
            }
        }
        DecryptedJoinAcceptPayload(self.0)
    }
}

/// DecryptedJoinAcceptPayload represents a decrypted JoinAccept.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [EncryptedJoinAcceptPayload.decrypt](struct.EncryptedJoinAcceptPayload.html#method.decrypt) function.
#[derive(Debug, PartialEq)]
pub struct DecryptedJoinAcceptPayload<T: AsRef<[u8]>>(T);

impl<T: AsRef<[u8]>> AsPhyPayloadBytes for DecryptedJoinAcceptPayload<T> {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> DecryptedJoinAcceptPayload<T> {
    /// Verifies that the JoinAccept has correct MIC.
    pub fn validate_mic(&self, key: &keys::AES128) -> bool {
        self.mic() == self.calculate_mic(key)
    }

    fn calculate_mic(&self, key: &keys::AES128) -> keys::MIC {
        let d = self.0.as_ref();
        securityhelpers::calculate_mic(&d[..d.len() - 4], key)
    }

    /// Gives the app nonce of the JoinAccept.
    pub fn app_nonce(&self) -> AppNonce<&[u8]> {
        AppNonce::new_from_raw(&self.0.as_ref()[1..4])
    }

    /// Gives the net ID of the JoinAccept.
    pub fn net_id(&self) -> NwkAddr<&[u8]> {
        NwkAddr::new_from_raw(&self.0.as_ref()[4..7])
    }

    /// Gives the dev address of the JoinAccept.
    pub fn dev_addr(&self) -> DevAddr<&[u8]> {
        DevAddr::new_from_raw(&self.0.as_ref()[7..11])
    }

    /// Gives the downlink configuration of the JoinAccept.
    pub fn dl_settings(&self) -> maccommands::DLSettings {
        maccommands::DLSettings::new(self.0.as_ref()[11])
    }

    /// Gives the RX delay of the JoinAccept.
    pub fn rx_delay(&self) -> u8 {
        self.0.as_ref()[12] & 0x0f
    }

    /// Gives the channel frequency list of the JoinAccept.
    pub fn c_f_list(&self) -> Vec<maccommands::Frequency> {
        if self.0.as_ref().len() == 17 {
            return Vec::new();
        }
        self.0.as_ref()[13..28]
            .chunks(3)
            .map(|f| maccommands::Frequency::new_from_raw(f))
            .collect()
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
    /// let app_key = lorawan::keys::AES128([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ///     0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let join_accept = lorawan::parser::DecryptedJoinAcceptPayload::new(data, &app_key).unwrap();
    ///
    /// let nwk_skey = join_accept.derive_newskey(
    ///     &lorawan::parser::DevNonce::new(&dev_nonce[..]).unwrap(),
    ///     &app_key,
    /// );
    /// ```
    pub fn derive_newskey<TT: AsRef<[u8]>>(&self, dev_nonce: &DevNonce<TT>, key: &keys::AES128) -> keys::AES128 {
        self.derive_session_key(0x1, dev_nonce, key)
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
    /// let app_key = lorawan::keys::AES128([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ///     0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let join_accept = lorawan::parser::DecryptedJoinAcceptPayload::new(data, &app_key).unwrap();
    ///
    /// let app_skey = join_accept.derive_appskey(
    ///     &lorawan::parser::DevNonce::new(&dev_nonce[..]).unwrap(),
    ///     &app_key,
    /// );
    /// ```
    pub fn derive_appskey<TT: AsRef<[u8]>>(&self, dev_nonce: &DevNonce<TT>, key: &keys::AES128) -> keys::AES128 {
        self.derive_session_key(0x2, dev_nonce, key)
    }

    fn derive_session_key<TT: AsRef<[u8]>>(&self,
        first_byte: u8,
        dev_nonce: &DevNonce<TT>,
        key: &keys::AES128) -> keys::AES128 {

        let key_arr = generic_array::GenericArray::from_slice(&key.0);
        let cipher = Aes128::new(key_arr);

        // note: AppNonce is 24 bit, NetId is 24 bit, DevNonce is 16 bit
        let app_nonce = self.app_nonce();
        let nwk_addr = self.net_id();
        let (app_nonce_arr, nwk_addr_arr, dev_nonce_arr)
            = (app_nonce.as_ref(), nwk_addr.as_ref(), dev_nonce.as_ref());

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

        let mut input = generic_array::GenericArray::clone_from_slice(&block);
        cipher.encrypt_block(&mut input);

        let mut output_key = [0u8; 16];
        output_key.copy_from_slice(&input[0..16]);
        keys::AES128(output_key)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> DecryptedJoinAcceptPayload<T> {
    /// Creates a DecryptedJoinAcceptPayload from the bytes of a JoinAccept.
    ///
    /// The JoinAccept payload is automatically decrypted and the mic is verified.
    ///
    /// # Argument
    ///
    /// * bytes - the data from which the PhyPayload is to be built.
    /// * key - the key that is to be used to decrypt the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut data = vec![0x20u8, 0x49u8, 0x3eu8, 0xebu8, 0x51u8, 0xfbu8,
    ///     0xa2u8, 0x11u8, 0x6fu8, 0x81u8, 0x0eu8, 0xdbu8, 0x37u8, 0x42u8,
    ///     0x97u8, 0x51u8, 0x42u8];
    /// let key = lorawan::keys::AES128([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
    ///     0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let phy = lorawan::parser::DecryptedJoinAcceptPayload::new(&mut data[..], &key);
    /// ```
    pub fn new<'a, 'b>(data: T, key: &'a keys::AES128) -> Result<Self, &'b str> {
        let t = EncryptedJoinAcceptPayload::new(data)?;
        let res = t.decrypt(key);
        if res.validate_mic(key) {
            Ok(res)
        } else {
            Err("MIC did not match")
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
    fn fhdr(&self) -> FHDR {
        FHDR::new_from_raw(&self.as_data_bytes()[1..(1 + self.fhdr_length())], self.is_uplink())
    }


    /// Gives whether the payload is uplink or not.
    fn is_uplink(&self) -> bool {
        let mhdr = MHDR(self.as_data_bytes()[0]);

        mhdr.mtype() == MType::UnconfirmedDataUp || mhdr.mtype() == MType::ConfirmedDataUp
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
        fhdr_length(FCtrl(self.as_data_bytes()[5], self.is_uplink()))
    }
}

fn fhdr_length(fctrl: FCtrl) -> usize {
    7 + fctrl.f_opts_len() as usize
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
#[derive(Debug, PartialEq)]
pub struct EncryptedDataPayload<T>(T);

impl<T: AsRef<[u8]>> DataHeader for EncryptedDataPayload<T> {
    fn as_data_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> EncryptedDataPayload<T> {
    /// Creates a PhyPayload from bytes.
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
    /// let phy = lorawan::parser::EncryptedDataPayload::new(data);
    /// ```
    pub fn new<'a>(data: T) -> Result<Self, &'a str> {
        //// TODO(ivaylo): Check this bug?
        //can_build = DataPayload::can_build_from(payload, false);
        if Self::can_build_from(data.as_ref()) {
            Ok(Self(data))
        } else {
            Err("can not build EncryptedDataPayload from the provided data")
        }
    }

    fn can_build_from(bytes: &[u8]) -> bool {
        let has_acceptable_len = bytes.len() >= 12 &&
            fhdr_length(FCtrl(bytes[5], true)) <= bytes.len();
        if !has_acceptable_len {
            return false;
        }
        match MHDR(bytes[0]).mtype() {
            MType::ConfirmedDataUp | MType::ConfirmedDataDown |
                MType::UnconfirmedDataUp | MType::UnconfirmedDataDown => {
                true
            }
            _ => {
                false
            }
        }
    }

    /// Verifies that the DataPayload has correct MIC.
    pub fn validate_mic(&self, key: &keys::AES128, fcnt: u32) -> bool {
        self.mic() == self.calculate_mic(key, fcnt)
    }

    fn calculate_mic(&self, key: &keys::AES128, fcnt: u32) -> keys::MIC {
        let d = self.0.as_ref();
        securityhelpers::calculate_data_mic(&d[..d.len() - 4], key, fcnt)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> EncryptedDataPayload<T> {
    /// Decrypts the EncryptedDataPayload payload.
    ///
    /// This method consumes the EncryptedDataPayload as it reuses the underlying memory. Please
    /// note that it does not verify the mic.
    ///
    /// # Argument
    ///
    /// * nwk_skey - the Network Session key used to decrypt the mac commands in case the payload
    ///     is transporting those.
    /// * app_skey - the Application Session key used to decrypt the application payload in case
    ///     the payload is transporting that.
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
    pub fn decrypt<'a, 'b>(mut self,
                   nwk_skey: Option<&'a keys::AES128>,
                   app_skey: Option<&'a keys::AES128>,
                   fcnt: u32) -> Result<DecryptedDataPayload<T>, &'b str> {
        let fhdr_length = self.fhdr_length();
        let fhdr = self.fhdr();
        let full_fcnt = compute_fcnt(fcnt, fhdr.fcnt());
        let key = if self.f_port().is_some() && self.f_port().unwrap() != 0{
            app_skey
        } else {
            nwk_skey
        };
        if key.is_none() {
            return Err("key needed to decrypt the frm data payload was None");
        }
        let data = self.0.as_ref();
        let clear_data = securityhelpers::encrypt_frm_data_payload(
            data,
            &data[(1 + fhdr_length + 1)..(data.len() - 4)],
            full_fcnt,
            &key.unwrap(),
        );
        let len = self.0.as_ref().len();

        self.0.as_mut()[(fhdr_length + 2)..(len - 4)].clone_from_slice(&clear_data[..]);
        Ok(DecryptedDataPayload(self.0))
    }
}

fn compute_fcnt(old_fcnt: u32, fcnt: u16) -> u32 {
    ((old_fcnt >> 16) << 16) ^ u32::from(fcnt)
}

/// DecryptedDataPayload represents a decrypted DataPayload.
///
/// It can be built either directly through the [new](#method.new) or using the
/// [EncryptedDataPayload.decrypt](struct.EncryptedDataPayload.html#method.decrypt) function.
#[derive(Debug, PartialEq)]
pub struct DecryptedDataPayload<T: AsRef<[u8]>>(T);

impl<T: AsRef<[u8]>> DataHeader for DecryptedDataPayload<T> {
    fn as_data_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> DecryptedDataPayload<T> {
    /// Returns FRMPayload that can represent either application payload or mac commands if fport
    /// is 0.
    pub fn frm_payload(&self) -> Result<FRMPayload, &str> {
        let data = self.as_data_bytes();
        let len = data.len();
        let fhdr_length = self.fhdr_length();
        //we have more bytes than fhdr + fport
        if len < fhdr_length + 6 {
            Ok(FRMPayload::None)
        } else if self.f_port() != Some(0) {
            // the size guarantees the existance of f_port
            Ok(FRMPayload::Data(&data[(1 + fhdr_length + 1)..(len - 4)]))
        } else {
            Ok(FRMPayload::MACCommands(FRMMacCommands::new(
                &data[(1 + fhdr_length + 1)..(len - 4)],
                self.is_uplink(),
            )))
        }
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> DecryptedDataPayload<T> {
    /// Creates a DecryptedDataPayload from the bytes of a DataPayload.
    ///
    /// The DataPayload payload is automatically decrypted and the mic is verified.
    ///
    /// # Argument
    ///
    /// * nwk_skey - the Network Session key used to decrypt the mac commands in case the payload
    ///     is transporting those.
    /// * app_skey - the Application Session key used to decrypt the application payload in case
    ///     the payload is transporting that.
    /// * fcnt - the counter used to encrypt the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut data = vec![0x40, 0x04, 0x03, 0x02, 0x01, 0x80, 0x01, 0x00, 0x01,
    ///     0xa6, 0x94, 0x64, 0x26, 0x15, 0xd6, 0xc3, 0xb5, 0x82];
    /// let nwk_skey = lorawan::keys::AES128([2; 16]);
    /// let app_skey = lorawan::keys::AES128([1; 16]);
    /// let dec_phy = lorawan::parser::DecryptedDataPayload::new(data,
    ///     &nwk_skey,
    ///     Some(&app_skey),
    ///     1).unwrap();
    /// ```
    pub fn new<'a, 'b>(data: T,
                   nwk_skey: &'a keys::AES128,
                   app_skey: Option<&'a keys::AES128>,
                   fcnt: u32) -> Result<Self, &'b str> {
        let t = EncryptedDataPayload::new(data)?;
        if !t.validate_mic(nwk_skey, fcnt) {
            return Err("invalid mic");
        }
        t.decrypt(Some(nwk_skey), app_skey, fcnt)
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
pub fn parse<'a, T: AsRef<[u8]> + AsMut<[u8]>>(data: T) -> Result<PhyPayload<T>, &'a str> {
    let bytes = data.as_ref();
    let len = bytes.len();
    // the smallest payload is a data payload without fport and FRMPayload
    // which is 12 bytes long.
    if len < 12 {
        return Err("insufficient number of bytes");
    }
    match MHDR(bytes[0]).mtype() {
        MType::JoinRequest => {
            Ok(PhyPayload::JoinRequest(JoinRequestPayload::new(data)?))
        },
        MType::JoinAccept => {
            Ok(PhyPayload::JoinAccept(JoinAcceptPayload::Encrypted(EncryptedJoinAcceptPayload::new(data)?)))
        },
        MType::UnconfirmedDataUp | MType::ConfirmedDataUp |
        MType::UnconfirmedDataDown | MType::ConfirmedDataDown => {
            Ok(PhyPayload::Data(DataPayload::Encrypted(EncryptedDataPayload::new(data)?)))
        },
        _ => Err("unsupported message type")
    }
}

/// MHDR represents LoRaWAN MHDR.
#[derive(Debug, PartialEq)]
pub struct MHDR(u8);

impl MHDR {
    pub fn new(byte: u8) -> MHDR {
        MHDR(byte)
    }

    /// Gives the type of message that PhyPayload is carrying.
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

    /// Gives the version of LoRaWAN payload format.
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
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
pub enum Major {
    LoRaWANR1,
    RFU,
}

fixed_len_struct! {
    /// EUI64 represents a 64 bit EUI.
    struct EUI64[8];
}

fixed_len_struct! {
    /// DevNonce represents a 16 bit device nonce.
    struct DevNonce[2];
}

fixed_len_struct! {
    /// AppNonce represents a 24 bit network server nonce.
    struct AppNonce[3];
}

fixed_len_struct! {
    /// DevAddr represents a 32 bit device address.
    struct DevAddr[4];
}

impl<T: AsRef<[u8]>> DevAddr<T> {
    pub fn nwk_id(&self) -> u8 {
        self.0.as_ref()[0] >> 1
    }
}

fixed_len_struct! {
    /// NwkAddr represents a 24 bit network address.
    struct NwkAddr[3];
}

/// FHDR represents FHDR from DataPayload.
#[derive(Debug, PartialEq)]
pub struct FHDR<'a>(&'a [u8], bool);

impl<'a> FHDR<'a> {
    pub fn new_from_raw(bytes: &'a [u8], uplink: bool) -> FHDR {
        FHDR(bytes, uplink)
    }

    pub fn new(bytes: &'a [u8], uplink: bool) -> Option<FHDR> {
        let data_len = bytes.len();
        if data_len < 7 {
            return None;
        }
        if data_len < fhdr_length(FCtrl(bytes[4], uplink)) {
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

    /// Gives the piggy-backed MAC ommands associated with the given payload.
    pub fn fopts(&self) -> Result<Vec<maccommands::MacCommand>, &str> {
        let f_opts_len = FCtrl(self.0[4], self.1).f_opts_len();
        maccommands::parse_mac_commands(&self.0[7 as usize..(7 + f_opts_len) as usize], self.1)
    }
}

/// FCtrl represents the FCtrl from FHDR.
#[derive(Debug, PartialEq)]
pub struct FCtrl(u8, bool);

impl FCtrl {
    pub fn new(bytes: u8, uplink: bool) -> FCtrl {
        FCtrl(bytes, uplink)
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
#[derive(Debug, PartialEq)]
pub enum FRMPayload<'a> {
    Data(&'a [u8]),
    MACCommands(FRMMacCommands<'a>),
    None,
}

/// FRMMacCommands represents the mac commands.
#[derive(Debug, PartialEq)]
pub struct FRMMacCommands<'a>(bool, &'a [u8]);

impl<'a> FRMMacCommands<'a> {
    pub fn new(bytes: &'a [u8], uplink: bool) -> Self {
        FRMMacCommands(uplink, bytes)
    }

    /// Gives the list of mac commands represented in the FRMPayload.
    pub fn mac_commands(&self) -> Result<Vec<maccommands::MacCommand>, &str> {
        maccommands::parse_mac_commands(self.1, self.0)
    }
}
