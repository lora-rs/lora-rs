// Copyright (c) 2017,2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use std::convert::AsRef;
use std::string::ToString;

use crypto::aessafe;
use crypto::symmetriccipher::BlockEncryptor;

use super::keys;
use super::maccommands;
use super::securityhelpers;

const INT_TO_HEX_MAP: &'static [u8] = b"0123456789abcdef";

macro_rules! fixed_len_struct {
    (
        $(#[$outer:meta])*
        struct $type:ident[$size:expr];
    ) => {
        $(#[$outer])*
        #[derive(Debug, PartialEq)]
        pub struct $type<'a>(&'a [u8; $size]);

        impl<'a> $type<'a> {
            fn new_from_raw(bytes: &'a [u8]) -> $type {
                $type(array_ref![bytes, 0, $size])
            }

            pub fn new(bytes: &'a [u8]) -> Option<$type> {
                if bytes.len() != $size {
                    None
                } else {
                    Some($type(array_ref![bytes, 0, $size]))
                }
            }
        }

        impl<'a> From<&'a [u8; $size]> for $type<'a> {
            fn from(v: &'a [u8; $size]) -> Self {
                $type(v)
            }
        }

        impl<'a> AsRef<[u8]> for $type<'a> {
            fn as_ref(&self) -> &[u8] {
                &self.0[..]
            }
        }

        impl<'a> ToString for $type<'a> {
            fn to_string(&self) -> String {
                let mut res = vec![0u8; 2 * $size];
                for i in 0..$size {
                    res[2 * i] = INT_TO_HEX_MAP[(self.0[i] >> 4) as usize];
                    res[2 * i + 1] = INT_TO_HEX_MAP[(self.0[i] & 0x0f) as usize];
                }

                unsafe { String::from_utf8_unchecked(res) }
            }
        }
    };
}

/// GenericPhyPayload contains the common logic for parsing the complete lorawan package for any
/// type that can be converted to bytes.
///
/// It has two versoins PhyPayload that uses borrowed bytes and PhysicalPayload that owns the bytes
/// for simpler passing around.
#[derive(Debug, PartialEq)]
pub struct GenericPhyPayload<T: AsRef<[u8]>>(T);

impl <'a, T: AsRef<[u8]>> GenericPhyPayload<T> {
    /// Creates a PhyPayload from bytes.
    ///
    /// # Argument
    ///
    /// * bytes - the data from which the PhyPayload is to be built.
    ///
    /// # Examples
    ///
    /// ```
    /// let data = vec![0x00u8, 0x04u8, 0x03u8, 0x02u8, 0x01u8, 0x04u8, 0x03u8,
    ///     0x02u8, 0x01u8, 0x05u8, 0x04u8, 0x03u8, 0x02u8, 0x05u8, 0x04u8,
    ///     0x03u8, 0x02u8, 0x2du8, 0x10u8, 0x6au8, 0x99u8, 0x0eu8, 0x12];
    /// let phy = lorawan::parser::PhyPayload::new(&data[..]);
    /// ```
    pub fn new(data: T) -> Result<GenericPhyPayload<T>, &'a str> {
        let result = GenericPhyPayload(data);
        {
            let bytes = result.0.as_ref();
            let len = bytes.len();
            // the smallest payload is a data payload without fport and FRMPayload
            // which is 12 bytes long.
            if len < 12 {
                return Err("insufficient number of bytes");
            }
            let can_build: bool;
            let payload = &bytes[1..(len - 4)];
            match result.mhdr().mtype() {
                MType::JoinRequest => {
                    can_build = JoinRequestPayload::can_build_from(payload);
                }
                MType::JoinAccept => {
                    can_build = JoinAcceptPayload::can_build_from(payload);
                }
                MType::UnconfirmedDataUp | MType::ConfirmedDataUp => {
                    can_build = DataPayload::can_build_from(payload, true);
                }
                MType::UnconfirmedDataDown | MType::ConfirmedDataDown => {
                    can_build = DataPayload::can_build_from(payload, true);
                }
                _ => return Err("unsupported message type"),
            }

            if !can_build {
                return Err("mac payload incorrect");
            }
        }

        Ok(result)
    }

    /// Creates a PhyPayload from the decrypted bytes of a JoinAccept.
    ///
    /// # Argument
    ///
    /// * bytes - the data from which the PhyPayload is to be built.
    /// * key - the key that is to be used to decrypt the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut data = vec![0x20u8, 0x49u8, 0x3eu8, 0xebu8, 0x51u8, 0xfbu8,
    ///     0xa2u8, 0x11u8, 0x6fu8, 0x81u8, 0x0eu8, 0xdbu8, 0x37u8, 0x42u8,
    ///     0x97u8, 0x51u8, 0x42u8];
    /// let key = lorawan::keys::AES128([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
    ///     0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let phy = lorawan::parser::PhyPayload::new_decrypted_join_accept(&mut data[..], &key);
    /// ```
    pub fn new_decrypted_join_accept<TT: AsRef<[u8]> + AsMut<[u8]>>(
        mut data: TT,
        key: &'a keys::AES128,
    ) -> Result<GenericPhyPayload<TT>, &'a str> {
        {
            let bytes = data.as_mut();
            let len = bytes.len();
            if len != 17 && len != 33 {
                return Err("bytes have incorrect size");
            }
            let aes_enc = aessafe::AesSafe128Encryptor::new(&key.0[..]);
            let mut tmp = vec![0; 16];
            for i in 0..(len >> 4) {
                let start = (i << 4) + 1;
                aes_enc.encrypt_block(&bytes[start..(start + 16)], &mut tmp[..]);
                for j in 0..16 {
                    bytes[start + j] = tmp[j];
                }
            }
        }
        GenericPhyPayload::new(data)
    }

    /// Gives the MHDR of the PhyPayload.
    pub fn mhdr(&self) -> MHDR {
        MHDR(self.0.as_ref()[0])
    }

    /// Gives the MIC of the PhyPayload.
    pub fn mic(&self) -> keys::MIC {
        let d = self.0.as_ref();
        let len = d.len();
        keys::MIC([
            d[len - 4],
            d[len - 3],
            d[len - 2],
            d[len - 1],
        ])
    }

    /// Gives the MacPayload of the PhyPayload.
    pub fn mac_payload(&self) -> MacPayload {
        let d = self.0.as_ref();
        let len = d.len();
        let bytes = &d[1..(len - 4)];
        match self.mhdr().mtype() {
            MType::JoinRequest => MacPayload::JoinRequest(JoinRequestPayload::new(bytes).unwrap()),
            MType::JoinAccept => MacPayload::JoinAccept(JoinAcceptPayload::new(bytes).unwrap()),
            MType::UnconfirmedDataUp | MType::ConfirmedDataUp => {
                MacPayload::Data(DataPayload::new(bytes, true).unwrap())
            }
            MType::UnconfirmedDataDown | MType::ConfirmedDataDown => {
                MacPayload::Data(DataPayload::new(bytes, true).unwrap())
            }
            _ => panic!("unexpected message type passed through the new method"),
        }
    }

    /// Verifies that the PhyPayload has correct MIC.
    ///
    /// The PhyPayload needs to contain DataPayload.
    pub fn validate_data_mic(&self, key: &keys::AES128, fcnt: u32) -> Result<bool, &str> {
        let expected_mic = self.calculate_data_mic(key, fcnt)?;
        let actual_mic = self.mic();

        Ok(actual_mic == expected_mic)
    }

    fn calculate_data_mic(&self, key: &keys::AES128, fcnt: u32) -> Result<keys::MIC, &str> {
        if let MacPayload::Data(_) = self.mac_payload() {
            let d = self.0.as_ref();
            Ok(securityhelpers::calculate_data_mic(
                &d[..d.len() - 4],
                key,
                fcnt,
            ))
        } else {
            Err("Could not read mac payload, maybe of incorrect type")
        }
    }

    /// Verifies that the PhyPayload has correct MIC.
    ///
    /// The PhyPayload needs to contain JoinRequest or JoinAccept.
    pub fn validate_join_mic(&self, key: &keys::AES128) -> Result<bool, &str> {
        let expected_mic = self.calculate_join_pkt_mic(key)?;
        let actual_mic = self.mic();

        Ok(actual_mic == expected_mic)
    }

    fn calculate_join_pkt_mic(&self, key: &keys::AES128) -> Result<keys::MIC, &str> {
        let mtype = self.mhdr().mtype();
        if mtype != MType::JoinRequest && mtype != MType::JoinAccept {
            return Err("Incorrect message type is not join request/accept");
        }

        let d = self.0.as_ref();
        Ok(securityhelpers::calculate_mic(
            &d[..d.len() - 4],
            key,
        ))
    }

    /// Decrypts the DataPayload payload.
    ///
    /// The PhyPayload needs to contain DataPayload.
    pub fn decrypted_payload(&self, key: &keys::AES128, fcnt: u32) -> Result<FRMPayload, &str> {
        if let MacPayload::Data(data_payload) = self.mac_payload() {
            let fhdr_length = data_payload.fhdr_length();
            let fhdr = data_payload.fhdr();
            let full_fcnt = compute_fcnt(fcnt, fhdr.fcnt());
            let clear_data = securityhelpers::encrypt_frm_data_payload(
                self.0.as_ref(),
                &data_payload.0[(fhdr_length + 1)..],
                full_fcnt,
                &key,
            );
            if clear_data.is_err() {
                return Err(clear_data.unwrap_err());
            }
            // we have more bytes than fhdr + fport
            if data_payload.0.len() <= fhdr_length + 1 {
                Err("insufficient number of bytes left")
            } else if data_payload.f_port() != Some(0) {
                // the size guarantees the existance of f_port
                Ok(FRMPayload::Data(clear_data.unwrap()))
            } else {
                Ok(FRMPayload::MACCommands(FRMMacCommands::new(
                    clear_data.unwrap(),
                    self.is_uplink(),
                )))
            }
        } else {
            Err("bad mac payload")
        }
    }

    fn is_uplink(&self) -> bool {
        let mhdr = self.mhdr();

        mhdr.mtype() == MType::UnconfirmedDataUp || mhdr.mtype() == MType::ConfirmedDataUp
    }
}

fn compute_fcnt(old_fcnt: u32, fcnt: u16) -> u32 {
    ((old_fcnt >> 16) << 16) ^ (fcnt as u32)
}

/// Represents the complete structure for handling lorawan mac layer payload.
///
/// See GenericPhyPayload documentation for more information.
pub type PhyPayload<'a> = GenericPhyPayload<&'a[u8]>;

/// Represents the complete structure for handling lorawan mac layer payload.
///
/// See GenericPhyPayload documentation for more information.
pub type PhysicalPayload = GenericPhyPayload<Vec<u8>>;

/// MHDR represents LoRaWAN MHDR.
#[derive(Debug, PartialEq)]
pub struct MHDR(u8);

impl MHDR {
    pub fn new(byte: u8) -> MHDR {
        MHDR(byte)
    }

    /// Gives the type of message that PhyPayload is carrying.
    pub fn mtype(&self) -> MType {
        match self.0 >> 5 {
            0 => MType::JoinRequest,
            1 => MType::JoinAccept,
            2 => MType::UnconfirmedDataUp,
            3 => MType::UnconfirmedDataDown,
            4 => MType::ConfirmedDataUp,
            5 => MType::ConfirmedDataDown,
            6 => MType::RFU,
            _ => MType::Proprietary,
        }
    }

    /// Gives the version of LoRaWAN payload format.
    pub fn major(&self) -> Major {
        if self.0 & 3 == 0 {
            Major::LoRaWANR1
        } else {
            Major::RFU
        }
    }
}

impl From<u8> for MHDR {
    fn from(v: u8) -> Self {
        MHDR(v)
    }
}

/// MType gives the possible message types of the PhyPayload.
#[derive(Debug, PartialEq)]
pub enum MType {
    JoinRequest,
    JoinAccept,
    UnconfirmedDataUp,
    UnconfirmedDataDown,
    ConfirmedDataUp,
    ConfirmedDataDown,
    RFU,
    Proprietary,
}

/// Major gives the supported LoRaWAN payload formats.
#[derive(Debug, PartialEq)]
pub enum Major {
    LoRaWANR1,
    RFU,
}

/// MacPayload represents all the possible mac payloads a PhyPayload can carry.
#[derive(Debug, PartialEq)]
pub enum MacPayload<'a> {
    Data(DataPayload<'a>),
    JoinRequest(JoinRequestPayload<'a>),
    JoinAccept(JoinAcceptPayload<'a>),
}

// *NOTE*: data should have at least 5 elements
fn fhdr_length<'a>(bytes: &'a [u8], uplink: bool) -> usize {
    7 + FCtrl(bytes[4], uplink).f_opts_len() as usize
}

/// DataPayload represents a data MacPayload.
#[derive(Debug, PartialEq)]
pub struct DataPayload<'a>(&'a [u8], bool);

impl<'a> DataPayload<'a> {
    /// Creates a DataPayload from data.
    ///
    /// # Argument
    ///
    /// * bytes - the data from which DataPayload is to be built.
    ///
    /// * uplink - whether the packet is uplink or downlink.
    ///
    /// # Examples
    ///
    /// ```
    /// let data = vec![0x04u8, 0x03u8, 0x02u8, 0x01u8, 0x04u8, 0x03u8, 0x02u8,
    ///     0x01u8, 0x05u8, 0x04u8, 0x03u8, 0x02u8, 0x05u8, 0x04u8, 0x03u8,
    ///     0x02u8, 0x2du8, 0x10u8];
    /// let data_payload = lorawan::parser::DataPayload::new(&data[..], true);
    /// ```
    pub fn new(bytes: &'a [u8], uplink: bool) -> Option<DataPayload> {
        if DataPayload::can_build_from(bytes, uplink) {
            Some(DataPayload(bytes, uplink))
        } else {
            None
        }
    }

    fn can_build_from(bytes: &'a [u8], uplink: bool) -> bool {
        bytes.len() >= 7 && fhdr_length(bytes, uplink) <= bytes.len()
    }

    /// Gives the FHDR of the DataPayload.
    pub fn fhdr(&self) -> FHDR {
        FHDR::new_from_raw(&self.0[0..self.fhdr_length()], self.1)
    }

    /// Gives the FPort of the DataPayload if there is one.
    pub fn f_port(&self) -> Option<u8> {
        let fhdr_length = self.fhdr_length();
        if fhdr_length + 1 >= self.0.len() {
            return None;
        }
        Some(self.0[self.fhdr_length()])
    }

    /// Gives the payload of the DataPayload if there is one.
    pub fn encrypted_frm_payload(&self) -> &'a [u8] {
        let fhdr_length = self.fhdr_length();
        if fhdr_length + 2 >= self.0.len() {
            return &self.0[0..0];
        }
        &self.0[(self.fhdr_length() + 1)..]
    }

    fn fhdr_length(&self) -> usize {
        fhdr_length(self.0, self.1)
    }
}

fixed_len_struct! {
    /// EUI64 represents a 64 bit EUI.
    struct EUI64[8];
}

fixed_len_struct! {
    /// DevNonce represents a 16 bit device nonce.
    struct DevNonce[2];
}

fixed_len_struct! {
    /// AppNonce represents a 24 bit network server nonce.
    struct AppNonce[3];
}

/// JoinRequestPayload represents a join request MacPayload.
#[derive(Debug, PartialEq)]
pub struct JoinRequestPayload<'a>(&'a [u8]);

impl<'a> JoinRequestPayload<'a> {
    pub fn new(bytes: &'a [u8]) -> Option<JoinRequestPayload> {
        if !JoinRequestPayload::can_build_from(bytes) {
            return None;
        }
        Some(JoinRequestPayload(bytes))
    }

    fn can_build_from(bytes: &'a [u8]) -> bool {
        bytes.len() == 18
    }

    pub fn app_eui(&self) -> EUI64 {
        EUI64::new_from_raw(&self.0[..8])
    }

    pub fn dev_eui(&self) -> EUI64 {
        EUI64::new_from_raw(&self.0[8..16])
    }

    pub fn dev_nonce(&self) -> DevNonce {
        DevNonce::new_from_raw(&self.0[16..18])
    }
}

/// JoinAcceptPayload represents a join accept MacPayload.
#[derive(Debug, PartialEq)]
pub struct JoinAcceptPayload<'a>(&'a [u8]);

impl<'a> JoinAcceptPayload<'a> {
    pub fn new(bytes: &'a [u8]) -> Option<JoinAcceptPayload> {
        if !JoinAcceptPayload::can_build_from(bytes) {
            return None;
        }

        Some(JoinAcceptPayload(bytes))
    }

    pub fn new_from_raw(bytes: &'a [u8]) -> JoinAcceptPayload {
        JoinAcceptPayload(bytes)
    }

    fn can_build_from(bytes: &'a [u8]) -> bool {
        let data_len = bytes.len();
        data_len == 12 || data_len == 28
    }

    pub fn app_nonce(&self) -> AppNonce {
        AppNonce::new_from_raw(&self.0[0..3])
    }

    pub fn net_id(&self) -> NwkAddr {
        NwkAddr::new_from_raw(&self.0[3..6])
    }

    pub fn dev_addr(&self) -> DevAddr {
        DevAddr::new_from_raw(&self.0[6..10])
    }

    /// Gives the downlink configuration.
    pub fn dl_settings(&self) -> maccommands::DLSettings {
        maccommands::DLSettings::new(self.0[10])
    }

    pub fn rx_delay(&self) -> u8 {
        self.0[11] & 0x0f
    }

    pub fn c_f_list(&self) -> Vec<maccommands::Frequency> {
        if self.0.len() == 12 {
            return Vec::new();
        }
        self.0[12..27]
            .chunks(3)
            .map(|f| maccommands::Frequency::new_from_raw(f))
            .collect()
    }
}

fixed_len_struct! {
    /// DevAddr represents a 32 bit device address.
    struct DevAddr[4];
}

impl<'a> DevAddr<'a> {
    pub fn nwk_id(&self) -> u8 {
        self.0[0] >> 1
    }
}

fixed_len_struct! {
    /// NwkAddr represents a 24 bit network address.
    struct NwkAddr[3];
}

/// FHDR represents FHDR from DataPayload.
#[derive(Debug, PartialEq)]
pub struct FHDR<'a>(&'a [u8], bool);

impl<'a> FHDR<'a> {
    pub fn new_from_raw(bytes: &'a [u8], uplink: bool) -> FHDR {
        FHDR(bytes, uplink)
    }

    pub fn new(bytes: &'a [u8], uplink: bool) -> Option<FHDR> {
        let data_len = bytes.len();
        if data_len < 7 {
            return None;
        }
        if data_len < fhdr_length(bytes, uplink) {
            return None;
        }
        Some(FHDR(bytes, uplink))
    }

    pub fn dev_addr(&self) -> DevAddr {
        DevAddr::new_from_raw(&self.0[0..4])
    }

    pub fn fctrl(&self) -> FCtrl {
        FCtrl(self.0[4], self.1)
    }

    pub fn fcnt(&self) -> u16 {
        let res = ((self.0[6] as u16) << 8) | (self.0[5] as u16);
        res
    }

    pub fn fopts(&self) -> Result<Vec<maccommands::MacCommand>, String> {
        let f_opts_len = FCtrl(self.0[4], self.1).f_opts_len();
        maccommands::parse_mac_commands(&self.0[7 as usize..(7 + f_opts_len) as usize], self.1)
    }
}

/// FCtrl represents the FCtrl from FHDR.
#[derive(Debug, PartialEq)]
pub struct FCtrl(u8, bool);

impl FCtrl {
    pub fn new(bytes: u8, uplink: bool) -> FCtrl {
        FCtrl(bytes, uplink)
    }

    pub fn adr(&self) -> bool {
        self.0 >> 7 == 1
    }

    pub fn adr_ack_req(&self) -> bool {
        self.1 && self.0 & (1 << 6) == 1
    }

    pub fn ack(&self) -> bool {
        self.0 & (1 << 5) == 1
    }

    pub fn f_pending(&self) -> bool {
        !self.1 && self.0 & (1 << 4) == 1
    }

    pub fn f_opts_len(&self) -> u8 {
        self.0 & 0x0f
    }

    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

/// FRMPayload represents the FRMPayload that can either be the application
/// data or mac commands.
#[derive(Debug, PartialEq)]
pub enum FRMPayload {
    Data(FRMDataPayload),
    MACCommands(FRMMacCommands),
}

/// FRMDataPayload represents the application data.
pub type FRMDataPayload = Vec<u8>;

/// FRMMacCommands represents the mac commands.
#[derive(Debug, PartialEq)]
pub struct FRMMacCommands(bool, Vec<u8>);

impl FRMMacCommands {
    pub fn new(bytes: Vec<u8>, uplink: bool) -> FRMMacCommands {
        FRMMacCommands(uplink, bytes)
    }

    pub fn mac_commands(&self) -> Result<Vec<maccommands::MacCommand>, String> {
        maccommands::parse_mac_commands(&self.1[..], self.0)
    }
}
