macro_rules! mac_cmd_zero_len {
    (
        $(
            $(#[$outer:meta])*
            struct $type:ident[cmd=$name:ident, cid=$cid:expr, uplink=$uplink:expr]
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

                /// Sent by end device or sent by network server.
                pub const fn uplink() -> bool {
                    $uplink
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
            struct $type:ident[cmd=$name:ident, cid=$cid:expr, uplink=$uplink:expr, size=$size:expr]
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

                /// Sent by end device or sent by network server.
                pub const fn uplink() -> bool {
                    $uplink
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

macro_rules! mac_cmds_enum {
    (
        $outer_vis:vis enum $outer_type:ident$(<$outer_lifetime:lifetime>),* {
        $(
            $name:ident($type:ident$(<$lifetime:lifetime>),*)
        )*
    }
    ) => {
        #[derive(Debug, PartialEq)]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
        #[allow(clippy::len_without_is_empty, missing_docs)]
        $outer_vis enum $outer_type$(<$outer_lifetime>)* {
            $(
                $name($type$(<$lifetime>)*),
            )*
        }
        impl$(<$outer_lifetime>)* $outer_type$(<$outer_lifetime>)* {
            /// Get the length.
            pub fn len(&self) -> usize {
                match *self {
                    $(
                        Self::$name(_) => $type::len(),
                    )*
                }
            }

            /// Sent by end device or sent by network server.
            pub fn uplink(&self) -> bool {
                match *self {
                    $(
                        Self::$name(_) => $type::uplink(),
                    )*
                }
            }

            /// Get reference to the data.
            pub fn bytes(&self) -> &[u8] {
                match *self {
                    $(
                        Self::$name(ref v) => v.bytes(),
                    )*
                }
            }
        }

        impl$(<$outer_lifetime>)* SerializableMacCommand for $outer_type$(<$outer_lifetime>)* {
            fn payload_bytes(&self) -> &[u8] {
                &self.bytes()
            }

            fn cid(&self) -> u8 {
                match *self {
                    $(
                        Self::$name(_) => $type::cid(),
                    )*
                }
            }

            fn payload_len(&self) -> usize {
                self.len()
            }
        }

        impl$(<$outer_lifetime>)* Iterator for MacCommandIterator<$($outer_lifetime)*, $outer_type$(<$outer_lifetime>)*> {
            type Item = $outer_type$(<$outer_lifetime>)*;

            fn next(&mut self) -> Option<Self::Item> {
                if self.index < self.data.len() {
                    let data = &self.data[self.index..];
                    $(
                        if data[0] == $type::cid() && data.len() >= $type::len() {
                            self.index = self.index + $type::len()+1;
                            Some($outer_type::$name($type::new_from_raw(&data[1.. 1 + $type::len()])))
                        } else
                    )* {
                        None
                    }
                }else{
                    None
                }
            }
        }

        impl<'a> From<&'a super::parser::FHDR<'a>>
            for MacCommandIterator<$($outer_lifetime)*, $outer_type$(<$outer_lifetime>)*>
        {
            fn from(fhdr: &'a super::parser::FHDR<'_>) -> Self {
                Self {
                    data: &fhdr.data(),
                    index: 0,
                    item: core::marker::PhantomData,
                }
            }
        }

        impl<'a> From<&'a super::parser::FRMMacCommands<'a>>
            for MacCommandIterator<$($outer_lifetime)*, $outer_type$(<$outer_lifetime>)*>
        {
            fn from(frmm: &'a super::parser::FRMMacCommands<'_>) -> Self {
                Self {
                    data: frmm.data(),
                    index: 0,
                    item: core::marker::PhantomData,
                }
            }
        }
    }
}

// Export the macros for internal use
pub(crate) use {mac_cmd_zero_len, mac_cmds, mac_cmds_enum};
