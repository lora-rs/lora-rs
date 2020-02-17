// Copyright (c) 2017,2018 Ivaylo Petrov
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
    let data_len = data.len();
    let mut mic_bytes = vec![0; data_len + 16];

    // compute b0 from the spec
    generate_helper_block(data, 0x49, fcnt, &mut mic_bytes[..16]);
    mic_bytes[15] = data.len() as u8;

    mic_bytes[16..].copy_from_slice(data);

    calculate_mic(&mic_bytes[..], key)
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

/// calculate_mic computes the MIC of a correct data packet.
pub fn calculate_mic<'a>(data: &'a [u8], key: &keys::AES128) -> keys::MIC {
    let mut cipher = Cmac::<Aes128>::new_varkey(&key.0[..]).unwrap();

    cipher.input(data);
    let result = cipher.result();

    let mut mic = [0u8; 4];
    mic.copy_from_slice(&result.code()[0..4]);

    keys::MIC(mic)
}

/// encrypt_frm_data_payload encrypts bytes
pub fn encrypt_frm_data_payload<'a>(
    phy_payload: &[u8],
    frm_payload: &[u8],
    fcnt: u32,
    key: &keys::AES128,
) -> Result<Vec<u8>, &'a str> {
    // make the block size a multiple of 16
    let block_size = ((frm_payload.len() + 15) / 16) * 16;
    let mut block = Vec::new();
    block.extend_from_slice(frm_payload);
    block.extend_from_slice(&vec![0u8; block_size - frm_payload.len()][..]);

    let mut a = [0u8; 16];
    generate_helper_block(phy_payload, 0x01, fcnt, &mut a[..]);

    let aes_enc = Aes128::new(GenericArray::from_slice(&key.0[..]));
    let mut result: Vec<u8> = block
        .chunks(16)
        .enumerate()
        .flat_map(|(i, c)| {
            a[15] = (i + 1) as u8;
            let mut tmp = GenericArray::from_mut_slice(&mut a[..]);
            aes_enc.encrypt_block(&mut tmp);
            c.iter()
                .enumerate()
                .map(|(j, v)| v ^ tmp[j])
                .collect::<Vec<u8>>()
        })
        .collect();

    result.truncate(frm_payload.len());

    Ok(result)
}
