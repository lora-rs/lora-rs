use crate::keys::*;
use crate::parser::*;

#[cfg(feature = "with-to-string")]
pub extern crate std;

pub use hex::FromHexError;

macro_rules! fixed_len_struct_impl_to_string_msb {
    (
        $type:ident,$size:expr;
    ) => {
        impl core::str::FromStr for $type {
            type Err = FromHexError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let mut res = [0; $size];
                hex::decode_to_slice(s.as_bytes(), &mut res)?;
                Ok(Self::from(res))
            }
        }

        #[cfg(feature = "with-to-string")]
        impl core::fmt::Display for $type {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut res = std::string::String::with_capacity($size * 2);
                res.extend(std::iter::repeat('-').take($size * 2));
                let slice = unsafe { &mut res.as_bytes_mut() };
                hex::encode_to_slice(self.as_ref(), slice).unwrap();
                write!(f, "{}", res)
            }
        }
    };
    (
        $type:ident[$size:expr];
    ) => {
        impl core::str::FromStr for $type<[u8; $size]> {
            type Err = FromHexError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let mut res = [0; $size];
                hex::decode_to_slice(s.as_bytes(), &mut res)?;
                Ok(Self::from(res))
            }
        }

        #[cfg(feature = "with-to-string")]
        impl<T: AsRef<[u8]>> core::fmt::Display for $type<T> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                // initialize the return string with the correct size and fill it with '-'
                let mut res = std::string::String::with_capacity($size * 2);
                res.extend(std::iter::repeat('-').take($size * 2));

                // get the string as bytes so that we can call hex::encode_to_slice directly
                let slice = unsafe { &mut res.as_bytes_mut() };
                hex::encode_to_slice(self.as_ref(), slice).unwrap();
                write!(f, "{}", res)
            }
        }
    };
}

macro_rules! fixed_len_struct_impl_string_lsb {
    (
        $type:ident,$size:expr;
    ) => {
        impl core::str::FromStr for $type {
            type Err = FromHexError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let mut res = [0; $size];
                hex::decode_to_slice(s.as_bytes(), &mut res)?;
                res.reverse();
                Ok(Self::from(res))
            }
        }

        #[cfg(feature = "with-to-string")]
        impl core::fmt::Display for $type {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut res = std::string::String::with_capacity($size * 2);
                res.extend(std::iter::repeat('0').take($size * 2));
                let slice = unsafe { &mut res.as_bytes_mut() };
                self.as_ref().iter().rev().enumerate().for_each(|(i, b)| {
                    hex::encode_to_slice(&[*b], &mut slice[i * 2..i * 2 + 2]).unwrap();
                });
                write!(f, "{}", res)
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

fixed_len_struct_impl_string_lsb! {
    DevEui, 8;
}

fixed_len_struct_impl_string_lsb! {
    AppEui, 8;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::extra::std::string::ToString;
    use core::str::FromStr;

    #[test]
    fn test_appskey_to_string() {
        let appskey = AppSKey::from([
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfd, 0xb9, 0x75, 0x31, 0x24, 0x68,
            0xac, 0xed,
        ]);
        assert_eq!(appskey.to_string(), "0123456789abcdeffdb975312468aced");
    }

    #[test]
    fn test_appskey_from_str() {
        let appskey = AppSKey::from_str("00112233445566778899aabbccddeeff").unwrap();
        assert_eq!(
            appskey,
            AppSKey::from([
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
                0xEE, 0xFF
            ])
        );
    }

    #[test]
    fn test_deveui_to_string() {
        let deveui = DevEui::from([0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12]);
        assert_eq!(deveui.to_string(), "123456789abcdef0");
    }

    #[test]
    fn test_deveui_from_str() {
        let deveui = DevEui::from_str("123456789abcdef0").unwrap();
        assert_eq!(deveui, DevEui::from([0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12]));
    }

    #[test]
    fn test_deveui_from_small_str() {
        let result = DevEui::from_str("123456789abcd");
        assert_eq!(result, Err(FromHexError::OddLength));
    }

    #[test]
    fn test_deveui_from_large_str() {
        let result = DevEui::from_str("123456789abcdef000");
        assert_eq!(result, Err(FromHexError::InvalidStringLength));
    }
}
