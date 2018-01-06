// Copyright (c) 2017,2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use crypto::aessafe;
use crypto::mac::Mac;
use crypto::symmetriccipher::BlockDecryptor;

use super::cmac;
use super::keys;
use super::parser;

/// JoinAcceptCreator serves for creating binary representation of Physical
/// Payload of JoinAccept.
pub struct JoinAcceptCreator {
    data: Vec<u8>,
    encrypted: bool,
}

impl JoinAcceptCreator {
    /// Creates a well initialized JoinAcceptCreator.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut phy = lorawan::creator::JoinAcceptCreator::new();
    /// let key = lorawan::keys::AES128([1; 16]);
    /// let app_nonce_bytes = [1; 3];
    /// phy.set_app_nonce(&app_nonce_bytes);
    /// phy.set_net_id([1; 3]);
    /// phy.set_dev_addr([1; 4]);
    /// phy.set_dl_settings(2);
    /// phy.set_rx_delay(1);
    /// let payload = phy.build(&key).unwrap();
    /// ```
    pub fn new() -> JoinAcceptCreator {
        let mut data = vec![0; 17];
        data[0] = 0x20;
        JoinAcceptCreator {
            data: data,
            encrypted: false,
        }
    }

    /// Sets the AppNonce of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * app_nonce - instance of lorawan::parser::AppNonce or anything that can
    ///   be converted into it.
    pub fn set_app_nonce<'a, T: Into<parser::AppNonce<'a>>>(&mut self, app_nonce: T) {
        let converted = app_nonce.into();
        self.data[1..4].copy_from_slice(converted.as_ref());
    }

    /// Sets the network ID of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * net_id - instance of lorawan::parser::NwkAddr or anything that can
    ///   be converted into it.
    pub fn set_net_id<T: Into<parser::NwkAddr>>(&mut self, net_id: T) {
        let converted = net_id.into();
        self.data[4] = converted.as_ref()[2];
        self.data[5] = converted.as_ref()[1];
        self.data[6] = converted.as_ref()[0];
    }

    /// Sets the device address of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_addr - instance of lorawan::parser::DevAddr or anything that can
    ///   be converted into it.
    pub fn set_dev_addr<T: Into<parser::DevAddr>>(&mut self, dev_addr: T) {
        let converted = dev_addr.into();
        self.data[7] = converted.as_ref()[3];
        self.data[8] = converted.as_ref()[2];
        self.data[9] = converted.as_ref()[1];
        self.data[10] = converted.as_ref()[0];
    }

    /// Sets the DLSettings of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * dl_settings - instance of lorawan::parser::DLSettings or anything
    ///   that can be converted into it.
    pub fn set_dl_settings<T: Into<parser::DLSettings>>(&mut self, dl_settings: T) {
        let converted = dl_settings.into();
        self.data[11] = converted.raw_value();
    }

    /// Sets the RX delay of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * rx_delay - the rx delay for the first receive window.
    pub fn set_rx_delay(&mut self, rx_delay: u8) {
        self.data[12] = rx_delay;
    }

    /// Sets the CFList of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * ch_list - list of Frequences to be sent to the device.
    pub fn set_c_f_list(&mut self, ch_list: Vec<parser::Frequency>) -> Result<bool, &str> {
        if ch_list.len() > 5 {
            return Err("too many frequences");
        }
        ch_list.iter().enumerate().for_each(|(i, fr)| {
            let v = fr.value() / 100;
            self.data[13 + i * 3] = (v & 0xff) as u8;
            self.data[14 + i * 3] = ((v >> 8) & 0xff) as u8;
            self.data[15 + i * 3] = ((v >> 16) & 0xff) as u8;
        });

        Ok(true)
    }

    /// Provides the binary representation of the encrypted join accept
    /// physical payload with the MIC set.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for encryption and setting the MIC.
    pub fn build(&mut self, key: &keys::AES128) -> Result<&[u8], &str> {
        if !self.encrypted {
            self.encrypt_payload(key);
        }
        Ok(&self.data[..])
    }

    fn encrypt_payload(&mut self, key: &keys::AES128) {
        set_mic(&mut self.data[..], key);
        let aes_enc = aessafe::AesSafe128Decryptor::new(&key.0[..]);
        let mut tmp = vec![0; 16];
        for i in 0..(self.data.len() >> 4) {
            let start = (i << 4) + 1;
            aes_enc.decrypt_block(&self.data[start..(start + 16)], &mut tmp[..]);
            for j in 0..16 {
                self.data[start + j] = tmp[j];
            }
        }
        self.encrypted = true;
    }
}

fn set_mic(data: &mut [u8], key: &keys::AES128) {
    let aes_enc = aessafe::AesSafe128Encryptor::new(&key.0[..]);
    let mut hasher = cmac::Cmac::new(aes_enc);

    let len = data.len();
    hasher.input(&data[..(len - 4)]);
    let r = hasher.result();
    let result = r.code();
    data[len - 4] = result[0];
    data[len - 3] = result[1];
    data[len - 2] = result[2];
    data[len - 1] = result[3];
}

/// JoinRequestCreator serves for creating binary representation of Physical
/// Payload of JoinRequest.
pub struct JoinRequestCreator {
    data: Vec<u8>,
}

impl JoinRequestCreator {
    /// Creates a well initialized JoinRequestCreator.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut phy = lorawan::creator::JoinRequestCreator::new();
    /// let key = lorawan::keys::AES128([7; 16]);
    /// phy.set_app_eui(&[1; 8]);
    /// phy.set_dev_eui(&[2; 8]);
    /// phy.set_dev_nonce(&[3; 2]);
    /// let payload = phy.build(&key).unwrap();
    /// ```
    pub fn new() -> JoinRequestCreator {
        let mut data = vec![0; 23];
        data[0] = 0x00;
        JoinRequestCreator { data: data }
    }

    /// Sets the application EUI of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * app_eui - instance of lorawan::parser::EUI64 or anything that can
    ///   be converted into it.
    pub fn set_app_eui<'a, T: Into<parser::EUI64<'a>>>(&mut self, app_eui: T) {
        let converted = app_eui.into();
        self.data[1..9].copy_from_slice(converted.as_ref());
    }

    /// Sets the device EUI of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_eui - instance of lorawan::parser::EUI64 or anything that can
    ///   be converted into it.
    pub fn set_dev_eui<'a, T: Into<parser::EUI64<'a>>>(&mut self, dev_eui: T) {
        let converted = dev_eui.into();
        self.data[9..17].copy_from_slice(converted.as_ref());
    }

    /// Sets the device nonce of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_nonce - instance of lorawan::parser::DevNonce or anything that can
    ///   be converted into it.
    pub fn set_dev_nonce<'a, T: Into<parser::DevNonce<'a>>>(&mut self, dev_nonce: T) {
        let converted = dev_nonce.into();
        self.data[17..19].copy_from_slice(converted.as_ref());
    }

    /// Provides the binary representation of the JoinRequest physical payload
    /// with the MIC set.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for setting the MIC.
    pub fn build(&mut self, key: &keys::AES128) -> Result<&[u8], &str> {
        set_mic(&mut self.data[..], key);
        Ok(&self.data[..])
    }
}
