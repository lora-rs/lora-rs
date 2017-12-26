// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! This module implements LoRaWAN packet handling and parsing.

#[macro_use]
extern crate arrayref;
extern crate crypto;

use crypto::aessafe;
use crypto::mac::Mac;
use crypto::symmetriccipher::BlockEncryptor;

pub mod cmac;


/// Represents the complete structure for handling lorawan mac layer payload.
#[derive(Debug, PartialEq)]
pub struct PhyPayload<'a>(&'a [u8]);

impl<'a> PhyPayload<'a> {
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
    /// let phy = lorawan::PhyPayload::new(&data[..]);
    /// ```
    pub fn new(bytes: &[u8]) -> Result<PhyPayload, &str> {
        // the smallest payload is a data payload without fport and FRMPayload
        // which is 12 bytes long.
        let len = bytes.len();
        if len < 12 {
            return Err("insufficient number of bytes");
        }
        let result = PhyPayload(bytes);
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
            MType::UnconfirmedDataDown |
            MType::ConfirmedDataDown => {
                can_build = DataPayload::can_build_from(payload, true);
            }
            _ => return Err("unsupported message type"),
        }

        if !can_build {
            return Err("mac payload incorrect");
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
    /// let key = lorawan::AES128([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
    ///     0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    /// let phy = lorawan::PhyPayload::new_decrypted_join_accept(&mut data[..], &key);
    /// ```
    pub fn new_decrypted_join_accept(
        bytes: &'a mut [u8],
        key: &'a AES128,
    ) -> Result<PhyPayload<'a>, &'a str> {
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
        PhyPayload::new(&bytes[..])
    }

    /// Gives the MHDR of the PhyPayload.
    pub fn mhdr(&self) -> MHDR {
        MHDR(self.0[0])
    }

    /// Gives the MIC of the PhyPayload.
    pub fn mic(&self) -> MIC {
        let len = self.0.len();
        MIC(
            [
                self.0[len - 4],
                self.0[len - 3],
                self.0[len - 2],
                self.0[len - 1],
            ],
        )
    }

    /// Gives the MacPayload of the PhyPayload.
    pub fn mac_payload(&self) -> MacPayload {
        let len = self.0.len();
        let bytes = &self.0[1..(len - 4)];
        match self.mhdr().mtype() {
            MType::JoinRequest => MacPayload::JoinRequest(JoinRequestPayload::new(bytes).unwrap()),
            MType::JoinAccept => MacPayload::JoinAccept(JoinAcceptPayload::new(bytes).unwrap()),
            MType::UnconfirmedDataUp | MType::ConfirmedDataUp => {
                MacPayload::Data(DataPayload::new(bytes, true).unwrap())
            }
            MType::UnconfirmedDataDown |
            MType::ConfirmedDataDown => MacPayload::Data(DataPayload::new(bytes, true).unwrap()),
            _ => panic!("unexpected message type passed through the new method"),
        }
    }

    /// Verifies that the PhyPayload has correct MIC.
    ///
    /// The PhyPayload needs to contain DataPayload.
    pub fn validate_data_mic(&self, key: &AES128, fcnt: u32) -> Result<bool, &str> {
        let expected_mic: MIC;

        match self.calculate_data_mic(key, fcnt) {
            Ok(mic) => {
                expected_mic = mic;
            }
            Err(e) => return Err(e),
        };
        let actual_mic = self.mic();

        Ok(actual_mic == expected_mic)
    }

    fn calculate_data_mic(&self, key: &AES128, fcnt: u32) -> Result<MIC, &str> {
        let payload_bytes = &self.0[..(self.0.len() - 4)];
        let mut b0 = [0u8; 16];
        b0[0] = 0x49;
        // b0[1..5] are 0
        let mhdr = self.mhdr();
        if mhdr.mtype() == MType::UnconfirmedDataDown || mhdr.mtype() == MType::ConfirmedDataDown {
            b0[5] = 1;
        }

        if let MacPayload::Data(mac_payload) = self.mac_payload() {
            let dev_addr = mac_payload.fhdr().dev_addr();
            b0[6] = dev_addr.0[3];
            b0[7] = dev_addr.0[2];
            b0[8] = dev_addr.0[1];
            b0[9] = dev_addr.0[0];
        } else {
            return Err("Could not read mac payload, maybe of incorrect type");
        }
        // fcnt
        b0[10] = (fcnt & 0xff) as u8;
        b0[11] = ((fcnt >> 8) & 0xff) as u8;
        b0[12] = ((fcnt >> 16) & 0xff) as u8;
        b0[13] = ((fcnt >> 24) & 0xff) as u8;
        // b0[14] is 0
        b0[15] = payload_bytes.len() as u8;

        let mut mic_bytes = Vec::new();
        mic_bytes.extend_from_slice(&b0[..]);
        mic_bytes.extend_from_slice(payload_bytes);

        let aes_enc = aessafe::AesSafe128Encryptor::new(&key.0[..]);
        let mut cmac1 = cmac::Cmac::new(aes_enc);

        cmac1.input(&mic_bytes[..]);
        let result = cmac1.result();
        let mut mic = [0u8; 4];
        mic.copy_from_slice(&result.code()[0..4]);

        Ok(MIC(mic))
    }

    /// Verifies that the PhyPayload has correct MIC.
    ///
    /// The PhyPayload needs to contain JoinRequest or JoinAccept.
    pub fn validate_join_mic(&self, key: &AES128) -> Result<bool, &str> {
        let expected_mic: MIC;

        match self.calculate_join_pkt_mic(key) {
            Ok(mic) => {
                expected_mic = mic;
            }
            Err(e) => return Err(e),
        };
        let actual_mic = self.mic();

        Ok(actual_mic == expected_mic)
    }

    fn calculate_join_pkt_mic(&self, key: &AES128) -> Result<MIC, &str> {
        let mtype = self.mhdr().mtype();
        if mtype != MType::JoinRequest && mtype != MType::JoinAccept {
            return Err("Incorrect message type is not join request/accept");
        }

        let mic_bytes = &self.0[..(self.0.len() - 4)];

        let aes_enc = aessafe::AesSafe128Encryptor::new(&key.0[..]);
        let mut cmac1 = cmac::Cmac::new(aes_enc);

        cmac1.input(mic_bytes);
        let result = cmac1.result();
        let mut mic = [0u8; 4];
        mic.copy_from_slice(&result.code()[0..4]);

        Ok(MIC(mic))
    }

    /// Decrypts the DataPayload payload.
    ///
    /// The PhyPayload needs to contain DataPayload.
    pub fn decrypted_payload(&self, key: &AES128, fcnt: u32) -> Result<FRMPayload, &str> {
        if let MacPayload::Data(data_payload) = self.mac_payload() {
            let fhdr_length = data_payload.fhdr_length();
            let fhdr = data_payload.fhdr();
            let dev_addr = fhdr.dev_addr();
            let full_fcnt = compute_fcnt(fcnt, fhdr.fcnt());
            let clear_data = self.encrypt_frm_data_payload(
                key,
                &dev_addr,
                full_fcnt,
                &data_payload.0[(fhdr_length + 1)..],
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
                Ok(FRMPayload::MACCommands(
                    FRMMacCommands::new(clear_data.unwrap(), self.is_uplink()),
                ))
            }
        } else {
            Err("bad mac payload")
        }
    }

    fn is_uplink(&self) -> bool {
        let mhdr = self.mhdr();

        mhdr.mtype() == MType::UnconfirmedDataUp || mhdr.mtype() == MType::ConfirmedDataUp
    }

    fn encrypt_frm_data_payload(
        &self,
        key: &AES128,
        dev_addr: &DevAddr,
        fcnt: u32,
        bytes: &[u8],
    ) -> Result<Vec<u8>, &str> {
        // make the block size a multiple of 16
        let block_size = ((bytes.len() + 15) / 16) * 16;
        let mut block = Vec::new();
        block.extend_from_slice(bytes);
        block.extend_from_slice(&vec![0u8; block_size - bytes.len()][..]);

        let mut a = [0u8; 16];
        a[0] = 0x01;
        a[5] = 1 - (self.is_uplink() as u8);
        a[6] = dev_addr.0[3];
        a[7] = dev_addr.0[2];
        a[8] = dev_addr.0[1];
        a[9] = dev_addr.0[0];
        a[10] = (fcnt & 0xff) as u8;
        a[11] = ((fcnt >> 8) & 0xff) as u8;
        a[12] = ((fcnt >> 16) & 0xff) as u8;
        a[13] = ((fcnt >> 24) & 0xff) as u8;

        let aes_enc = aessafe::AesSafe128Encryptor::new(&key.0[..]);
        let mut result: Vec<u8> = block
            .chunks(16)
            .enumerate()
            .flat_map(|(i, c)| {
                let mut tmp = [0u8; 16];
                a[15] = (i + 1) as u8;
                aes_enc.encrypt_block(&a[..], &mut tmp);
                c.iter()
                    .enumerate()
                    .map(|(j, v)| v ^ tmp[j])
                    .collect::<Vec<u8>>()
            })
            .collect();

        result.truncate(bytes.len());

        Ok(result)
    }
}

fn compute_fcnt(old_fcnt: u32, fcnt: u16) -> u32 {
    ((old_fcnt >> 16) << 16) ^ (fcnt as u32)
}

/// MHDR represents LoRaWAN MHDR.
#[derive(Debug, PartialEq)]
pub struct MHDR(pub u8);

impl MHDR {
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

/// AES128 represents 128 bit AES key.
#[derive(Debug, PartialEq)]
pub struct AES128(pub [u8; 16]);

/// MIC represents LoRaWAN MIC.
#[derive(Debug, PartialEq)]
pub struct MIC(pub [u8; 4]);

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
    /// let data_payload = lorawan::DataPayload::new(&data[..], true);
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
    pub fn encrypted_from_payload(&self) -> &'a [u8] {
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

/// EUI64 represents a 64 bit EUI.
#[derive(Debug, PartialEq)]
pub struct EUI64<'a>(&'a [u8; 8]);

impl<'a> EUI64<'a> {
    fn new_from_raw(bytes: &'a [u8]) -> EUI64 {
        EUI64(array_ref![bytes, 0, 8])
    }

    pub fn new(bytes: &'a [u8]) -> Option<EUI64> {
        if bytes.len() != 8 {
            None
        } else {
            Some(EUI64(array_ref![bytes, 0, 8]))
        }
    }
}

/// DevNonce represents a 16 bit device nonce.
#[derive(Debug, PartialEq)]
pub struct DevNonce<'a>(&'a [u8; 2]);

impl<'a> DevNonce<'a> {
    fn new_from_raw(bytes: &'a [u8]) -> DevNonce {
        DevNonce(array_ref![bytes, 0, 2])
    }

    pub fn new(bytes: &'a [u8]) -> Option<DevNonce> {
        if bytes.len() != 2 {
            None
        } else {
            Some(DevNonce(array_ref![bytes, 0, 2]))
        }
    }
}

/// AppNonce represents a 24 bit network server nonce.
#[derive(Debug, PartialEq)]
pub struct AppNonce<'a>(&'a [u8; 3]);

impl<'a> AppNonce<'a> {
    fn new_from_raw(bytes: &'a [u8]) -> AppNonce {
        AppNonce(array_ref![bytes, 0, 3])
    }

    pub fn new(bytes: &'a [u8]) -> Option<AppNonce> {
        if bytes.len() != 3 {
            None
        } else {
            Some(AppNonce(array_ref![bytes, 0, 3]))
        }
    }
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
        NwkAddr([self.0[5], self.0[4], self.0[3]])
    }

    pub fn dev_addr(&self) -> DevAddr {
        DevAddr([self.0[9], self.0[8], self.0[7], self.0[6]])
    }

    /// Gives the downlink configuration.
    pub fn dl_settings(&self) -> DLSettings {
        DLSettings(self.0[10])
    }

    pub fn rx_delay(&self) -> u8 {
        self.0[11]
    }

    pub fn c_f_list(&self) -> Vec<Frequency> {
        if self.0.len() == 12 {
            return Vec::new();
        }
        self.0[12..27]
            .chunks(3)
            .map(|f| Frequency::new_from_raw(f))
            .collect()
    }
}

/// DLSettings represents LoRaWAN MHDR.
#[derive(Debug, PartialEq)]
pub struct DLSettings(pub u8);

impl DLSettings {
    pub fn rx1_dr_offset(&self) -> u8 {
        self.0 >> 4 & 0x07
    }

    pub fn rx2_data_rate(&self) -> u8 {
        self.0 & 0x0f
    }
}

#[derive(Debug, PartialEq)]
pub struct Frequency<'a>(&'a [u8]);

impl<'a> Frequency<'a> {
    pub fn new_from_raw(bytes: &'a [u8]) -> Frequency {
        Frequency(bytes)
    }

    pub fn new(bytes: &'a [u8]) -> Option<Frequency> {
        if bytes.len() != 3 {
            return None;
        }

        Some(Frequency(bytes))
    }

    pub fn value(&self) -> u32 {
        (((self.0[2] as u32) << 16) + ((self.0[1] as u32) << 8) + (self.0[0] as u32)) * 100
    }
}

/// DevAddr represents a 32 bit device address.
#[derive(Debug, PartialEq)]
pub struct DevAddr([u8; 4]);

impl DevAddr {
    pub fn new(bytes: &[u8; 4]) -> DevAddr {
        DevAddr([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    pub fn nwk_id(&self) -> u8 {
        self.0[0] >> 1
    }
}

/// NwkAddr represents a 24 bit network address.
#[derive(Debug, PartialEq)]
pub struct NwkAddr(pub [u8; 3]);

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
        DevAddr([self.0[3], self.0[2], self.0[1], self.0[0]])
    }

    pub fn fctrl(&self) -> FCtrl {
        FCtrl(self.0[4], self.1)
    }

    pub fn fcnt(&self) -> u16 {
        let res = ((self.0[6] as u16) << 8) | (self.0[5] as u16);
        res
    }

    pub fn fopts(&self) -> Vec<MacCommand> {
        let res = Vec::new();

        res
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

    pub fn mac_commands(&self) -> Vec<MacCommand> {
        Vec::new()
    }
}

/// MacCommand represents the enumeration of all LoRaWAN MACCommands.
pub enum MacCommand<'a> {
    LinkCheckReq(LinkCheckReqPayload<'a>),
    // TODO(ivajloip): Finish :)
    //LinkCheckAns
    //LinkADRReq
    //LinkADRAns
    //DutyCycleReq
    //DutyCycleAns
    //RXParamSetupReq
    //RXParamSetupAns
    //DevStatusReq
    //DevStatusAns
    //NewChannelReq
    //NewChannelAns
    //RXTimingSetupReq
    //RXTimingSetupAns
    //Proprietary
}

/// LinkCheckReqPayload represents the LinkCheckReq LoRaWAN MACCommand.
#[derive(Debug, PartialEq)]
pub struct LinkCheckReqPayload<'a>(&'a [u8; 2]);
