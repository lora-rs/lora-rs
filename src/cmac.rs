// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Taken from: https://github.com/a-dma/rust-crypto/commit/ab498b6585334d9331de4bca4c42a5193bd2bd8e

extern crate crypto;

/*
 * This module implements the CMAC function - a Message Authentication Code using symmetric encryption.
 */

use std::iter::repeat;

use crypto::mac::{Mac, MacResult};
use crypto::symmetriccipher::BlockEncryptor;

/// The CMAC struct represents a CMAC function - a Message Authentication Code using symmetric
/// encryption.
pub struct Cmac<C: BlockEncryptor> {
    cipher: C,
    key_one: Vec<u8>,
    key_two: Vec<u8>,
    result: Vec<u8>,
    finished: bool,
}

fn do_shift_one_bit_left(a: &[u8], block_size: usize) -> (Vec<u8>, u8) {
    let mut carry = 0;

    let mut b: Vec<u8> = repeat(0).take(block_size).collect();

    for i in (0..(block_size)).rev() {
        b[i] = (a[i] << 1) | carry;

        if a[i] & 0x80 != 0 {
            carry = 1;
        } else {
            carry = 0;
        }
    }

    (b, carry)
}

fn generate_subkey(key: &[u8], block_size: usize) -> Vec<u8> {
    let (mut subkey, carry) = do_shift_one_bit_left(key, block_size);

    // Only two block sizes are defined, 64 and 128
    let r_b = if block_size == 16 { 0x87 } else { 0x1b };

    if carry == 1 {
        subkey[block_size - 1] ^= r_b;
    }

    subkey
}

// Cmac uses two keys derived from the provided key
fn create_keys<C: BlockEncryptor>(cipher: &C) -> (Vec<u8>, Vec<u8>) {
    let zeroes: Vec<u8> = repeat(0).take(cipher.block_size()).collect();
    let mut l: Vec<u8> = repeat(0).take(cipher.block_size()).collect();

    cipher.encrypt_block(zeroes.as_slice(), l.as_mut_slice());

    let key_one = generate_subkey(l.as_slice(), cipher.block_size());
    let key_two = generate_subkey(key_one.as_slice(), cipher.block_size());

    (key_one, key_two)
}

fn do_inplace_xor(a: &[u8], b: &mut [u8]) {
    for (x, y) in a.iter().zip(b) {
        *y ^= *x;
    }
}

fn do_pad(data: &mut [u8], len: usize, block_size: usize) {
    data[len] = 0x80;

    for i in (len + 1)..block_size {
        data[i] = 0x00;
    }
}

// Perform simil-CBC encryption with last block tweaking
fn cmac_encrypt<C: BlockEncryptor>(
    cipher: &C,
    key_one: &[u8],
    key_two: &[u8],
    data: &[u8],
) -> Vec<u8> {
    let block_size = cipher.block_size();

    let n_blocks = if data.len() == 0 {
        0
    } else {
        (data.len() + (block_size - 1)) / block_size - 1
    };

    let remaining_bytes = data.len() % block_size;

    let (head, tail) = if n_blocks == 0 {
        (&[] as &[u8], data)
    } else {
        data.split_at(block_size * n_blocks)
    };

    let mut mac: Vec<u8> = repeat(0).take(block_size).collect();
    let mut work_block: Vec<u8> = Vec::with_capacity(block_size);

    for block in head.chunks(block_size) {
        do_inplace_xor(block, mac.as_mut_slice());

        work_block.clone_from(&mac);
        cipher.encrypt_block(work_block.as_slice(), mac.as_mut_slice());
    }

    work_block.truncate(0);
    if remaining_bytes == 0 {
        if data.len() != 0 {
            work_block.extend_from_slice(tail);
            do_inplace_xor(key_one, work_block.as_mut_slice());
        } else {
            work_block = repeat(0).take(block_size).collect();
            do_pad(work_block.as_mut_slice(), 0, block_size);
            do_inplace_xor(key_two, work_block.as_mut_slice());
        }
    } else {
        work_block.extend_from_slice(tail);
        work_block.extend_from_slice(vec![0; block_size - remaining_bytes].as_slice()); // NOTE(adma): try to use a FixedBuffer
        do_pad(work_block.as_mut_slice(), remaining_bytes, block_size);
        do_inplace_xor(key_two, work_block.as_mut_slice());
    }

    do_inplace_xor(work_block.as_slice(), mac.as_mut_slice());

    cipher.encrypt_block(mac.as_slice(), work_block.as_mut_slice());

    work_block
}

impl<C: BlockEncryptor> Cmac<C> {
    /// Create a new CMAC instance.
    /// # Arguments
    /// * cipher - The Cipher to use.
    pub fn new(cipher: C) -> Cmac<C> {
        let (key_one, key_two) = create_keys(&cipher);

        Cmac {
            result: Vec::with_capacity(cipher.block_size()), // NOTE(adma): try to use a FixedBuffer
            cipher: cipher,
            key_one: key_one,
            key_two: key_two,
            finished: false,
        }
        // NOTE(adma): cipher should be either AES or TDEA
    }
}

impl<C: BlockEncryptor> Mac for Cmac<C> {
    fn input(&mut self, data: &[u8]) {
        assert!(!self.finished);
        self.result = cmac_encrypt(
            &self.cipher,
            self.key_one.as_slice(),
            self.key_two.as_slice(),
            data,
        );
        self.finished = true;
    }

    fn reset(&mut self) {
        self.finished = false;
    }

    fn result(&mut self) -> MacResult {
        let output_size = self.cipher.block_size();
        let mut code: Vec<u8> = repeat(0).take(output_size).collect();

        self.raw_result(&mut code);

        MacResult::new_from_owned(code)
    }

    fn raw_result(&mut self, output: &mut [u8]) {
        if !self.finished {
            output.clone_from_slice(&[]);
        }

        output.clone_from_slice(self.result.as_slice());
    }

    fn output_bytes(&self) -> usize {
        self.cipher.block_size()
    }
}
