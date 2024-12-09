macro_rules! mac_cmd_zero_len {
    (
        $(
            $(#[$outer:meta])*
            struct $type:ident[cmd=$name:ident, cid=$cid:expr]
            )*
    ) => {
        $(
            $(#[$outer])*
            pub struct $type();

            impl $type {
                /// Creation.
                pub fn new(_: &[u8]) -> $type {
                    $type()
                }

                /// Duplicate fn to be compatible with the mac_cmds macro
                pub fn new_from_raw(_: &[u8]) ->$type {
                    $type()
                }

                /// Get the CID.
                pub const fn cid() -> u8 {
                    $cid
                }

                /// Length of empty payload.
                pub const fn len() -> usize {
                    0
                }

                /// Reference to the empty payload.
                pub fn bytes (&self) -> &[u8]{
                    &[]
                }
            }
        )*
    }
}

macro_rules! mac_cmds {
    (
        $(
            $(#[$outer:meta])*
            struct $type:ident[cmd=$name:ident, cid=$cid:expr, size=$size:expr]
            )*
    ) => {
        $(
            $(#[$outer])*
            pub struct $type<'a>(pub(crate) &'a [u8]);

            impl<'a> $type<'a> {
                /// Creates a new instance of the MAC command if there is enought data.
                pub fn new(data: &'a [u8]) -> Result<$type<'a>, Error> {
                    if data.len() != $size {
                        Err(Error::BufferTooShort)
                    } else {
                        Ok($type(&data))
                    }
                }
                /// Constructs a new instance of the MAC command from the provided data,
                /// without verifying the data length.
                ///
                /// Improper use of this method could lead to panic during runtime!
                pub fn new_from_raw(data: &'a [u8]) ->$type<'a> {
                    $type(&data)
                }

                /// Get the CID.
                pub const fn cid() -> u8 {
                    $cid
                }

                /// Length of payload without the CID.
                pub const fn len() -> usize {
                    $size
                }

                /// Reference to the payload.
                pub fn bytes (&self) -> &[u8]{
                    self.0
                }
            }
        )*
    }
}

// Export the macros for internal use
pub(crate) use {mac_cmd_zero_len, mac_cmds};
