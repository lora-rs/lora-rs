// Copyright (c) 2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

pub extern crate std;
use crate::parser::*;
use crate::keys::*;

macro_rules! fixed_len_struct_impl_to_string_msb {
    (
        $type:ident,$size:expr;
    ) => {
        impl std::string::ToString for $type {
            fn to_string(&self) -> std::string::String {
                let mut res = std::string::String::with_capacity($size * 2);
                res.extend(std::iter::repeat('0').take($size * 2));
                let slice = unsafe { &mut res.as_bytes_mut() };
                hex::encode_to_slice(self.as_ref(), slice).unwrap();
                res
            }
        }
    };
    (
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

macro_rules! fixed_len_struct_impl_to_string_lsb {
    (
        $type:ident,$size:expr;
    ) => {
        impl std::string::ToString for $type {
            fn to_string(&self) -> std::string::String {
                let mut res = std::string::String::with_capacity($size * 2);
                res.extend(std::iter::repeat('0').take($size * 2));
                let slice = unsafe { &mut res.as_bytes_mut() };
                self.as_ref().iter().rev().enumerate().for_each(|(i, b)| {
                    hex::encode_to_slice(&[*b], &mut slice[i*2..i*2+2]).unwrap();
                });
                res
            }
        }
    };
}

fixed_len_struct_impl_to_string_msb! {
    EUI64[8];
}

fixed_len_struct_impl_to_string_msb! {
    DevNonce[2];
}

fixed_len_struct_impl_to_string_msb! {
    AppNonce[3];
}

fixed_len_struct_impl_to_string_msb! {
    DevAddr[4];
}

fixed_len_struct_impl_to_string_msb! {
    NwkAddr[3];
}

fixed_len_struct_impl_to_string_msb! {
    AppKey, 16;
}

fixed_len_struct_impl_to_string_msb! {
    NewSKey, 16;
}

fixed_len_struct_impl_to_string_msb! {
    AppSKey, 16;
}

fixed_len_struct_impl_to_string_lsb! {
    DevEui, 8;
}

fixed_len_struct_impl_to_string_lsb! {
    AppEui, 8;
}



#[cfg(test)]
mod test {
    use super::*;
    use crate::extra::std::string::ToString;

    #[test]
    fn test_appskey_to_string() {
        let appskey = AppSKey::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        assert_eq!(appskey.to_string(), "00112233445566778899aabbccddeeff");
    }

    #[test]
    fn test_deveui_to_string() {
        let deveui = DevEui::from([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]);
        assert_eq!(deveui.to_string(), "7766554433221100");
    }
}