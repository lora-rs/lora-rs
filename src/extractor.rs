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

pub struct JoinAcceptCreator {
    data: Vec<u8>,
    encrypted: bool,
}

impl JoinAcceptCreator {
    pub fn new() -> JoinAcceptCreator {
        let mut data = vec![0; 17];
        data[0] = 0x20;
        JoinAcceptCreator {
            data: data,
            encrypted: false,
        }
    }

    pub fn set_app_nonce<'a, T: Into<parser::AppNonce<'a>>>(&mut self, app_nonce: T) {
        let converted = app_nonce.into();
        self.data[1] = converted.as_ref()[0];
        self.data[2] = converted.as_ref()[1];
        self.data[3] = converted.as_ref()[2];
    }

    pub fn set_net_id<T: Into<parser::NwkAddr>>(&mut self, net_id: T) {
        let converted = net_id.into();
        self.data[4] = converted.as_ref()[2];
        self.data[5] = converted.as_ref()[1];
        self.data[6] = converted.as_ref()[0];
    }

    pub fn set_dev_addr<T: Into<parser::DevAddr>>(&mut self, dev_addr: T) {
        let converted = dev_addr.into();
        self.data[7] = converted.as_ref()[3];
        self.data[8] = converted.as_ref()[2];
        self.data[9] = converted.as_ref()[1];
        self.data[10] = converted.as_ref()[0];
    }

    pub fn set_dl_settings<T: Into<parser::DLSettings>>(&mut self, dl_settings: T) {
        let converted = dl_settings.into();
        self.data[11] = converted.raw_value();
    }

    pub fn set_rx_delay(&mut self, rx_delay: u8) {
        self.data[12] = rx_delay;
    }

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

    pub fn build(&mut self, key: keys::AES128) -> Result<&[u8], &str> {
        if !self.encrypted {
            let aes_enc = aessafe::AesSafe128Encryptor::new(&key.0[..]);
            let mut hasher = cmac::Cmac::new(aes_enc);

            let len = self.data.len();
            hasher.input(&self.data[..(len - 4)]);
            let r = hasher.result();
            let result = r.code();
            self.data[len - 4] = result[0];
            self.data[len - 3] = result[1];
            self.data[len - 2] = result[2];
            self.data[len - 1] = result[3];
            let aes_enc = aessafe::AesSafe128Decryptor::new(&key.0[..]);
            let mut tmp = vec![0; 16];
            for i in 0..(len >> 4) {
                let start = (i << 4) + 1;
                aes_enc.decrypt_block(&self.data[start..(start + 16)], &mut tmp[..]);
                for j in 0..16 {
                    self.data[start + j] = tmp[j];
                }
            }
            self.encrypted = true;
        }
        Ok(&self.data[..])
    }
}
