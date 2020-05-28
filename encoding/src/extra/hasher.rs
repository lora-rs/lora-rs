// Copyright (c) 2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

pub extern crate std;

use std::hash::{Hash, Hasher};

use crate::parser::*;

macro_rules! fixed_len_struct_impl_hash {
    (
        $(#[$outer:meta])*
        $type:ident[$size:expr];
    ) => {
        impl<T: AsRef<[u8]>> Hash for $type<T> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.as_ref().hash(state);
            }
        }
    };
}

fixed_len_struct_impl_hash! {
    EUI64[8];
}

fixed_len_struct_impl_hash! {
    DevNonce[2];
}

fixed_len_struct_impl_hash! {
    AppNonce[3];
}

fixed_len_struct_impl_hash! {
    DevAddr[4];
}

fixed_len_struct_impl_hash! {
    NwkAddr[3];
}
