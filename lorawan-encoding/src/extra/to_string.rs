// Copyright (c) 2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

pub extern crate std;
use crate::parser::*;

macro_rules! fixed_len_struct_impl_to_string {
    (
        $(#[$outer:meta])*
        $type:ident[$size:expr];
    ) => {
        impl<T: AsRef<[u8]>> std::string::ToString for $type<T> {
            fn to_string(&self) -> std::string::String {
                let mut res = std::string::String::with_capacity($size * 2);
                res.extend(std::iter::repeat('0').take($size * 2));
                let slice = unsafe { &mut res.as_bytes_mut() };
                hex::encode_to_slice(self.as_ref(), slice).unwrap();
                res
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
