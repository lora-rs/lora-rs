// Copyright (c) 2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

pub extern crate std;

use crate::parser::*;

const INT_TO_HEX_MAP: &[u8] = b"0123456789abcdef";

macro_rules! fixed_len_struct_impl_to_string {
    (
        $(#[$outer:meta])*
        $type:ident[$size:expr];
    ) => {
        impl<T: AsRef<[u8]>> std::string::ToString for $type<T> {
            fn to_string(&self) -> std::string::String {
                let mut res = std::vec::Vec::new();
                let data = self.as_ref();
                res.extend_from_slice(&[0; 2 * $size]);
                for i in 0..$size {
                    res[2 * i] = INT_TO_HEX_MAP[(data[i] >> 4) as usize];
                    res[2 * i + 1] = INT_TO_HEX_MAP[(data[i] & 0x0f) as usize];
                }

                unsafe { std::string::String::from_utf8_unchecked(res) }
            }
        }
    };
}

fixed_len_struct_impl_to_string! {
    EUI64[8];
}

fixed_len_struct_impl_to_string! {
    DevNonce[2];
}

fixed_len_struct_impl_to_string! {
    AppNonce[3];
}

fixed_len_struct_impl_to_string! {
    DevAddr[4];
}

fixed_len_struct_impl_to_string! {
    NwkAddr[3];
}
