//! Provides a default software implementation for LoRaWAN's cryptographic functions.
use super::creator::JoinRequestCreator;
use super::keys::*;
use super::parser::{
    DecryptedDataPayload, DecryptedJoinAcceptPayload, EncryptedDataPayload,
    EncryptedJoinAcceptPayload, JoinRequestPayload,
};
use crate::creator::{DataPayloadCreator, JoinAcceptCreator};
use crate::parser::Error;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;

pub type Cmac = cmac::Cmac<Aes128>;

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

impl<D: AsMut<[u8]>> DataPayloadCreator<D, DefaultFactory> {
    /// Creates a well initialized DataPayloadCreator.
    ///
    /// By default the packet is unconfirmed data up packet.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut buf = [0u8; 255];
    /// let mut phy = lorawan::creator::DataPayloadCreator::new(&mut buf).unwrap();
    /// let nwk_skey = lorawan::keys::NwkSKey::from([2; 16]);
    /// let app_skey = lorawan::keys::AppSKey::from([1; 16]);
    /// let fctrl = lorawan::parser::FCtrl::new(0x80, true);
    /// phy.set_confirmed(false).
    ///     set_uplink(true).
    ///     set_f_port(1).
    ///     set_dev_addr(&[4, 3, 2, 1]).
    ///     set_fctrl(&fctrl). // ADR: true, all others: false
    ///     set_fcnt(1);
    /// let payload = phy.build(b"hello", &[], &nwk_skey, &app_skey).unwrap();
    /// ```
    pub fn new(buf: D) -> Result<Self, super::creator::Error> {
        Self::with_options(buf, DefaultFactory)
    }
}

impl<D: AsMut<[u8]>> JoinRequestCreator<D, DefaultFactory> {
    /// Creates a well initialized JoinRequestCreator.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut buf = [0u8; 23];
    /// let mut phy = lorawan::creator::JoinRequestCreator::new(&mut buf[..]).unwrap();
    /// let key = lorawan::keys::AppKey::from([7; 16]);
    /// phy.set_app_eui(&[1; 8]);
    /// phy.set_dev_eui(&[2; 8]);
    /// phy.set_dev_nonce(&[3; 2]);
    /// let payload = phy.build(&key);
    /// ```
    pub fn new(buf: D) -> Result<Self, super::creator::Error> {
        Self::with_options(buf, DefaultFactory)
    }
}

impl<D: AsMut<[u8]>> JoinAcceptCreator<D, DefaultFactory> {
    /// Creates a well initialized JoinAcceptCreator.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut buf = [0u8; 17];
    /// let mut phy = lorawan::creator::JoinAcceptCreator::new(&mut buf[..]).unwrap();
    /// let key = lorawan::keys::AES128([1; 16]);
    /// let app_nonce_bytes = [1; 3];
    /// phy.set_app_nonce(&app_nonce_bytes);
    /// phy.set_net_id(&[1; 3]);
    /// phy.set_dev_addr(&[1; 4]);
    /// phy.set_dl_settings(2);
    /// phy.set_rx_delay(1);
    /// let mut freqs: Vec<lorawan::types::Frequency> = Vec::new();
    /// freqs.push(lorawan::types::Frequency::new(&[0x58, 0x6e, 0x84,]).unwrap());
    /// freqs.push(lorawan::types::Frequency::new(&[0x88, 0x66, 0x84,]).unwrap());
    /// phy.set_c_f_list(freqs);
    /// let payload = phy.build(&key).unwrap();
    /// ```
    pub fn new(buf: D) -> Result<Self, super::creator::Error> {
        Self::with_options(buf, DefaultFactory)
    }
}

impl<T: AsRef<[u8]>> JoinRequestPayload<T, DefaultFactory> {
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
    pub fn new(data: T) -> Result<Self, Error> {
        Self::new_with_factory(data, DefaultFactory)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> EncryptedJoinAcceptPayload<T, DefaultFactory> {
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
    pub fn new(data: T) -> Result<Self, Error> {
        Self::new_with_factory(data, DefaultFactory)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> DecryptedJoinAcceptPayload<T, DefaultFactory> {
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
    /// let key = lorawan::keys::AppKey::from([2; 16]);
    /// let phy = lorawan::parser::DecryptedJoinAcceptPayload::new(&mut data[..], &key);
    /// ```
    pub fn new(data: T, key: &AppKey) -> Result<Self, Error> {
        Self::new_with_factory(data, key, DefaultFactory)
    }
}

impl<T: AsRef<[u8]>> EncryptedDataPayload<T, DefaultFactory> {
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
    pub fn new(data: T) -> Result<Self, Error> {
        Self::new_with_factory(data, DefaultFactory)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> DecryptedDataPayload<T> {
    /// Creates a DecryptedDataPayload from the bytes of a DataPayload.
    ///
    /// The DataPayload payload is automatically decrypted and the mic is verified.
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
    /// let nwk_skey = lorawan::keys::AES128([2; 16]);
    /// let app_skey = lorawan::keys::AES128([1; 16]);
    /// let dec_phy = lorawan::parser::DecryptedDataPayload::new(data,
    ///     &nwk_skey,
    ///     Some(&app_skey),
    ///     1).unwrap();
    /// ```
    pub fn new<'a>(
        data: T,
        nwk_skey: &'a AES128,
        app_skey: Option<&'a AES128>,
        fcnt: u32,
    ) -> Result<Self, Error> {
        let t = EncryptedDataPayload::new(data)?;
        if !t.validate_mic(nwk_skey, fcnt) {
            return Err(Error::InvalidMic);
        }
        t.decrypt(Some(nwk_skey), app_skey, fcnt)
    }
}
