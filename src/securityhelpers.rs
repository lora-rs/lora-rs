// Copyright (c) 2017,2018,2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use aes::block_cipher_trait::generic_array::GenericArray;
use aes::block_cipher_trait::BlockCipher;
use aes::Aes128;

use cmac::{Cmac, Mac}; 

use super::keys;

/// calculate_data_mic computes the MIC of a correct data packet.
pub fn calculate_data_mic<'a>(data: &'a [u8], key: &keys::AES128, fcnt: u32) -> keys::MIC {
    let mut header = [0; 16];

    // compute b0 from the spec
    generate_helper_block(data, 0x49, fcnt, &mut header[..16]);
    header[15] = data.len() as u8;

    calculate_mic_with_header(&header[..], data, key)
}

fn generate_helper_block(data: &[u8], first: u8, fcnt: u32, res: &mut [u8]) {
    res[0] = first;
    // res[1..5] are 0
    res[5] = (data[0] & 0x20) >> 5;
    res[6..10].copy_from_slice(&data[1..5]);
    // fcnt
    res[10] = (fcnt & 0xff) as u8;
    res[11] = ((fcnt >> 8) & 0xff) as u8;
    res[12] = ((fcnt >> 16) & 0xff) as u8;
    res[13] = ((fcnt >> 24) & 0xff) as u8;
    // res[14] is 0
    // res[15] is to be set later
}

fn calculate_mic_with_header<'a, 'b>(header: &'a [u8], data: &'b [u8], key: &keys::AES128) -> keys::MIC {
    let mut cipher = Cmac::<Aes128>::new_varkey(&key.0[..]).unwrap();

    cipher.input(header);
    cipher.input(data);
    let result = cipher.result();

    let mut mic = [0u8; 4];
    mic.copy_from_slice(&result.code()[0..4]);

    keys::MIC(mic)
}

/// calculate_mic computes the MIC of a correct data packet.
pub fn calculate_mic<'a>(data: &'a [u8], key: &keys::AES128) -> keys::MIC {
    calculate_mic_with_header(&[], data, key)
}

/// encrypt_frm_data_payload encrypts bytes
pub fn encrypt_frm_data_payload(
    phy_payload: &mut [u8],
    start: usize,
    end: usize,
    fcnt: u32,
    key: &keys::AES128,
) {
    let len = end - start;

    let mut a = [0u8; 16];
    generate_helper_block(phy_payload, 0x01, fcnt, &mut a[..]);

    let aes_enc = Aes128::new(GenericArray::from_slice(&key.0[..]));

    let mut tmp = GenericArray::from_mut_slice(&mut a[..]);
    for i in 0..len {
        let j = i & 0x0f;
        if j == 0 {
            a[15] = (i + 1) as u8;
            tmp = GenericArray::from_mut_slice(&mut a[..]);
            aes_enc.encrypt_block(&mut tmp);
        }
        phy_payload[start + i] ^= tmp[j]
    }
}
