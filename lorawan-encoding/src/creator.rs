//! Provides types and methods for creating LoRaWAN payloads.
//!
//! See [JoinAcceptCreator.new](struct.JoinAcceptCreator.html#method.new) for an example.
use super::keys::{AppKey, AppSKey, CryptoFactory, Decrypter, NwkSKey, AES128};
use super::maccommands::{mac_commands_len, SerializableMacCommand};
use super::parser;
use super::securityhelpers;
use crate::packet_length::phy::join::{
    JOIN_ACCEPT_LEN, JOIN_ACCEPT_WITH_CFLIST_LEN, JOIN_REQUEST_LEN,
};
use crate::packet_length::phy::mac::fhdr::FOPTS_MAX_LEN;
use crate::packet_length::phy::{MIC_LEN, PHY_PAYLOAD_MIN_LEN};
use crate::types::{DLSettings, Frequency};

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error {
    BufferTooShort,
    InvalidChannelList,
    MacCommandTooBigForFOpts,
    DataAndMacCommandsInPayloadNotAllowed,
    FRMPayloadWithFportZero,
}

/// Helper trait to provide dummy Creator implementation for
/// variable length commands.
#[allow(clippy::len_without_is_empty)]
pub trait UnimplementedCreator {
    fn new() -> Self
    where
        Self: Sized,
    {
        unimplemented!()
    }

    fn build(&self) -> &[u8] {
        unimplemented!()
    }

    fn len(&self) -> usize {
        unimplemented!()
    }
}

const PIGGYBACK_MAC_COMMANDS_MAX_LEN: usize = 15;

/// JoinAcceptCreator serves for creating binary representation of Physical
/// Payload of JoinAccept.
///
/// # Examples
///
/// ```
/// let mut buf = [0u8; 100];
/// let mut phy = lorawan::creator::JoinAcceptCreator::new(&mut buf).unwrap();
/// let key = lorawan::keys::AES128([1; 16]);
/// let app_nonce_bytes = [1; 3];
/// phy.set_app_nonce(&app_nonce_bytes);
/// phy.set_net_id(&[1; 3]);
/// phy.set_dev_addr(&[1; 4]);
/// phy.set_dl_settings(2);
/// phy.set_rx_delay(1);
/// let mut freqs: Vec<lorawan::types::Frequency> = Vec::new();
/// freqs.push(lorawan::types::Frequency::new(&[0x58, 0x6e, 0x84]).unwrap());
/// freqs.push(lorawan::types::Frequency::new(&[0x88, 0x66, 0x84]).unwrap());
/// phy.set_c_f_list(freqs);
/// let payload = phy.build(&key, &lorawan::default_crypto::DefaultFactory).unwrap();
/// ```
#[derive(Default)]
pub struct JoinAcceptCreator<D> {
    data: D,
    with_c_f_list: bool,
    encrypted: bool,
}

impl<D: AsMut<[u8]>> JoinAcceptCreator<D> {
    /// Creates a well initialized JoinAcceptCreator with specific data and crypto functions.
    ///
    /// TODO: Add more details & and example
    pub fn new(mut data: D) -> Result<Self, Error> {
        let d = data.as_mut();
        if d.len() < JOIN_ACCEPT_LEN {
            return Err(Error::BufferTooShort);
        }
        d[0] = 0x20;
        Ok(Self { data, with_c_f_list: false, encrypted: false })
    }

    /// Sets the AppNonce of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * app_nonce - instance of lorawan::parser::AppNonce or anything that can be converted into
    ///   it.
    pub fn set_app_nonce<H: AsRef<[u8]>, T: Into<parser::AppNonce<H>>>(
        &mut self,
        app_nonce: T,
    ) -> &mut Self {
        let converted = app_nonce.into();
        self.data.as_mut()[1..4].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the network ID of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * net_id - instance of lorawan::parser::NwkAddr or anything that can be converted into it.
    pub fn set_net_id<H: AsRef<[u8]>, T: Into<parser::NwkAddr<H>>>(
        &mut self,
        net_id: T,
    ) -> &mut Self {
        let converted = net_id.into();
        self.data.as_mut()[4..7].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the device address of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_addr - instance of lorawan::parser::DevAddr or anything that can be converted into it.
    pub fn set_dev_addr<H: AsRef<[u8]>, T: Into<parser::DevAddr<H>>>(
        &mut self,
        dev_addr: T,
    ) -> &mut Self {
        let converted = dev_addr.into();
        self.data.as_mut()[7..11].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the DLSettings of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * dl_settings - instance of lorawan::maccommands::DLSettings or anything that can be
    ///   converted into it.
    pub fn set_dl_settings<T: Into<DLSettings>>(&mut self, dl_settings: T) -> &mut Self {
        let converted = dl_settings.into();
        self.data.as_mut()[11] = converted.raw_value();

        self
    }

    /// Sets the RX delay of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * rx_delay - the rx delay for the first receive window.
    pub fn set_rx_delay(&mut self, rx_delay: u8) -> &mut Self {
        self.data.as_mut()[12] = rx_delay;

        self
    }

    /// Sets the CFList of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * ch_list - list of Frequences to be sent to the device.
    pub fn set_c_f_list<'a, C: AsRef<[Frequency<'a>]>>(
        &mut self,
        list: C,
    ) -> Result<&mut Self, Error> {
        let ch_list = list.as_ref();
        if ch_list.len() > 5 {
            return Err(Error::InvalidChannelList);
        }
        let d = self.data.as_mut();
        if d.len() < JOIN_ACCEPT_WITH_CFLIST_LEN {
            return Err(Error::BufferTooShort);
        }
        ch_list.iter().enumerate().for_each(|(i, fr)| {
            let v = fr.value() / 100;
            d[13 + i * 3] = (v & 0xff) as u8;
            d[14 + i * 3] = ((v >> 8) & 0xff) as u8;
            d[15 + i * 3] = ((v >> 16) & 0xff) as u8;
        });
        // set cflist type
        d[JOIN_ACCEPT_WITH_CFLIST_LEN - 1] = 0;
        self.with_c_f_list = true;

        Ok(self)
    }

    /// Provides the binary representation of the encrypted join accept
    /// physical payload with the MIC set.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for encryption and setting the MIC.
    pub fn build<F: CryptoFactory>(&mut self, key: &AES128, factory: &F) -> Result<&[u8], Error> {
        let required_len = if self.with_c_f_list {
            JOIN_ACCEPT_WITH_CFLIST_LEN
        } else {
            JOIN_ACCEPT_LEN
        };
        if self.data.as_mut().len() < required_len {
            return Err(Error::BufferTooShort);
        }
        if !self.encrypted {
            let d = if self.with_c_f_list {
                &mut self.data.as_mut()[..JOIN_ACCEPT_WITH_CFLIST_LEN]
            } else {
                &mut self.data.as_mut()[..JOIN_ACCEPT_LEN]
            };
            set_mic(d, key, factory);
            let aes_enc = factory.new_dec(key);
            for i in 0..(d.len() >> 4) {
                let start = (i << 4) + 1;
                aes_enc.decrypt_block(&mut d[start..(16 + start)]);
            }
            self.encrypted = true;
        }
        Ok(&self.data.as_mut()[..required_len])
    }
}

fn set_mic<F: CryptoFactory>(data: &mut [u8], key: &AES128, factory: &F) {
    let len = data.len();
    let mic = securityhelpers::calculate_mic(&data[..len - MIC_LEN], factory.new_mac(key));

    data[len - MIC_LEN..].copy_from_slice(&mic.0[..]);
}

/// JoinRequestCreator serves for creating binary representation of Physical
/// Payload of JoinRequest.
/// # Examples
///
/// ```
/// let mut buf = [0u8; 100];
/// let mut phy = lorawan::creator::JoinRequestCreator::new(&mut buf).unwrap();
/// let key = lorawan::keys::AppKey::from([7; 16]);
/// phy.set_app_eui(&[1; 8]);
/// phy.set_dev_eui(&[2; 8]);
/// phy.set_dev_nonce(&[3; 2]);
/// let payload = phy.build(&key, &lorawan::default_crypto::DefaultFactory);
/// ```
#[derive(Default)]
pub struct JoinRequestCreator<D> {
    data: D,
}

impl<D: AsMut<[u8]>> JoinRequestCreator<D> {
    /// Creates a well initialized JoinRequestCreator with specific crypto functions.
    pub fn new(mut data: D) -> Result<Self, Error> {
        let d = data.as_mut();
        if d.len() < JOIN_REQUEST_LEN {
            return Err(Error::BufferTooShort);
        }
        d[0] = 0x00;
        Ok(Self { data })
    }

    /// Sets the application EUI of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * app_eui - instance of lorawan::parser::EUI64 or anything that can be converted into it.
    pub fn set_app_eui<H: AsRef<[u8]>, T: Into<parser::EUI64<H>>>(
        &mut self,
        app_eui: T,
    ) -> &mut Self {
        let converted = app_eui.into();
        self.data.as_mut()[1..9].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the device EUI of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_eui - instance of lorawan::parser::EUI64 or anything that can be converted into it.
    pub fn set_dev_eui<H: AsRef<[u8]>, T: Into<parser::EUI64<H>>>(
        &mut self,
        dev_eui: T,
    ) -> &mut Self {
        let converted = dev_eui.into();
        self.data.as_mut()[9..17].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the device nonce of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_nonce - instance of lorawan::parser::DevNonce or anything that can be converted into
    ///   it.
    pub fn set_dev_nonce<T: Into<parser::DevNonce>>(&mut self, dev_nonce: T) -> &mut Self {
        let converted = dev_nonce.into();
        self.data.as_mut()[17..19].copy_from_slice(&converted.as_ref());

        self
    }

    /// Provides the binary representation of the JoinRequest physical payload
    /// with the MIC set.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for setting the MIC.
    pub fn build<F: CryptoFactory>(&mut self, key: &AppKey, factory: &F) -> &[u8] {
        let d = self.data.as_mut();
        set_mic(&mut d[..JOIN_REQUEST_LEN], &key.0, factory);
        &d[..JOIN_REQUEST_LEN]
    }
}

/// DataPayloadCreator serves for creating binary representation of Physical
/// Payload of DataUp or DataDown messages.
///
/// # Example
///
/// ```
/// let mut buf = [0u8; 23];
/// let mut phy = lorawan::creator::DataPayloadCreator::new(&mut buf[..]).unwrap();
/// let nwk_skey = lorawan::keys::NwkSKey::from([2; 16]);
/// let app_skey = lorawan::keys::AppSKey::from([1; 16]);
/// phy.set_confirmed(true)
///     .set_uplink(true)
///     .set_f_port(42)
///     .set_dev_addr(&[4, 3, 2, 1])
///     .set_fctrl(&lorawan::parser::FCtrl::new(0x80, true)) // ADR: true, all others: false
///     .set_fcnt(76543);
/// phy.build(b"hello lora", &[], &nwk_skey, &app_skey, &lorawan::default_crypto::DefaultFactory)
///     .unwrap();
/// ```
#[derive(Default)]
pub struct DataPayloadCreator<D> {
    data: D,
    data_f_port: Option<u8>,
    fcnt: u32,
}

impl<D: AsMut<[u8]>> DataPayloadCreator<D> {
    /// Creates a well initialized DataPayloadCreator with specific crypto functions.
    ///
    /// By default the packet is unconfirmed data up packet.
    pub fn new(mut data: D) -> Result<Self, Error> {
        let d = data.as_mut();
        if d.len() < PHY_PAYLOAD_MIN_LEN {
            return Err(Error::BufferTooShort);
        }
        d[0] = 0x40;
        Ok(DataPayloadCreator { data, data_f_port: None, fcnt: 0 })
    }

    /// Sets whether the packet is uplink or downlink.
    ///
    /// # Argument
    ///
    /// * uplink - whether the packet is uplink or downlink.
    pub fn set_uplink(&mut self, uplink: bool) -> &mut Self {
        if uplink {
            self.data.as_mut()[0] &= 0xdf;
        } else {
            self.data.as_mut()[0] |= 0x20;
        }
        self
    }

    /// Sets whether the packet is confirmed or unconfirmed.
    ///
    /// # Argument
    ///
    /// * confirmed - whether the packet is confirmed or unconfirmed.
    pub fn set_confirmed(&mut self, confirmed: bool) -> &mut Self {
        let d = self.data.as_mut();
        if confirmed {
            d[0] &= 0xbf;
            d[0] |= 0x80;
        } else {
            d[0] &= 0x7f;
            d[0] |= 0x40;
        }

        self
    }

    /// Sets the device address of the DataPayload to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_addr - instance of lorawan::parser::DevAddr or anything that can be converted into it.
    pub fn set_dev_addr<H: AsRef<[u8]>, T: Into<parser::DevAddr<H>>>(
        &mut self,
        dev_addr: T,
    ) -> &mut Self {
        let converted = dev_addr.into();
        self.data.as_mut()[1..5].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the FCtrl header of the DataPayload packet to the specified value.
    ///
    /// # Argument
    ///
    /// * fctrl - the FCtrl to be set.
    pub fn set_fctrl(&mut self, fctrl: &parser::FCtrl) -> &mut Self {
        self.data.as_mut()[5] = fctrl.raw_value();
        self
    }

    /// Sets the FCnt header of the DataPayload packet to the specified value.
    ///
    /// NOTE: In the packet header the value will be truncated to u16.
    ///
    /// # Argument
    ///
    /// * fctrl - the FCtrl to be set.
    pub fn set_fcnt(&mut self, fcnt: u32) -> &mut Self {
        let d = self.data.as_mut();
        self.fcnt = fcnt;
        d[6] = (fcnt & (0xff_u32)) as u8;
        d[7] = (fcnt >> 8) as u8;

        self
    }

    /// Sets the FPort header of the DataPayload packet to the specified value.
    ///
    /// If f_port == 0, automatically sets `encrypt_mac_commands` to `true`.
    ///
    /// # Argument
    ///
    /// * f_port - the FPort to be set.
    pub fn set_f_port(&mut self, f_port: u8) -> &mut Self {
        self.data_f_port = Some(f_port);

        self
    }

    /// Whether a set of mac commands can be piggybacked.
    pub fn can_piggyback(cmds: &[&dyn SerializableMacCommand]) -> bool {
        mac_commands_len(cmds) <= PIGGYBACK_MAC_COMMANDS_MAX_LEN
    }

    /// Provides the binary representation of the DataPayload physical payload
    /// with the MIC set and payload encrypted.
    ///
    /// # Argument
    ///
    /// * payload - the FRMPayload (application) to be sent.
    /// * nwk_skey - the key to be used for setting the MIC and possibly for MAC command encryption.
    /// * app_skey - the key to be used for payload encryption if fport not 0, otherwise nwk_skey is
    ///   only used.
    ///
    ///
    /// # Example
    ///
    /// ```
    /// use lorawan::{
    ///     maccommands::UplinkMacCommand,
    ///     maccommandcreator::{build_mac_commands, LinkADRAnsCreator},
    ///     packet_length::phy::mac::fhdr::FOPTS_MAX_LEN,
    /// };
    /// use heapless::Vec;
    ///
    /// let mut buf = [0u8; 255];
    /// let mut phy = lorawan::creator::DataPayloadCreator::new(&mut buf[..]).unwrap();
    /// let link_check_req = UplinkMacCommand::LinkCheckReq(
    ///     lorawan::maccommands::LinkCheckReqPayload(),
    /// );
    /// let mut link_adr_ans = LinkADRAnsCreator::new();
    /// let mut mac_cmds : Vec<u8, FOPTS_MAX_LEN> = Vec::new();
    /// mac_cmds.extend_from_slice(link_check_req.bytes()).unwrap();
    /// mac_cmds.extend_from_slice(link_adr_ans.build()).unwrap();
    /// let nwk_skey = lorawan::keys::NwkSKey::from([2; 16]);
    /// let app_skey = lorawan::keys::AppSKey::from([1; 16]);
    /// phy.build(&[], mac_cmds.as_slice(), &nwk_skey, &app_skey, &lorawan::default_crypto::DefaultFactory).unwrap();
    /// ```
    pub fn build<F: CryptoFactory, M: AsRef<[u8]>>(
        &mut self,
        payload: &[u8],
        mac_cmds: M,
        nwk_skey: &NwkSKey,
        app_skey: &AppSKey,
        factory: &F,
    ) -> Result<&[u8], Error> {
        let d = self.data.as_mut();
        let mut last_filled = 8; // MHDR + FHDR without the FOpts
        let has_fport = self.data_f_port.is_some();
        let has_fport_zero = has_fport && self.data_f_port.unwrap() == 0;
        let mac_cmds_len = mac_cmds.as_ref().len();
        // Set MAC Commands
        if mac_cmds_len > FOPTS_MAX_LEN && !has_fport_zero {
            return Err(Error::MacCommandTooBigForFOpts);
        }

        // Set FPort
        let mut payload_len = payload.len();

        if has_fport_zero && payload_len > 0 {
            return Err(Error::DataAndMacCommandsInPayloadNotAllowed);
        }
        if !has_fport && payload_len > 0 {
            return Err(Error::FRMPayloadWithFportZero);
        }
        // Set FOptsLen if present
        if !has_fport_zero && mac_cmds_len > 0 {
            if d.len() < last_filled + mac_cmds_len + MIC_LEN {
                return Err(Error::BufferTooShort);
            }
            d[5] |= mac_cmds_len as u8 & 0x0f;
            // copy mac commmands into d
            d[last_filled..last_filled + mac_cmds_len].copy_from_slice(mac_cmds.as_ref());
            last_filled += mac_cmds_len;
        }

        if has_fport {
            if d.len() < last_filled + 1 + MIC_LEN {
                return Err(Error::BufferTooShort);
            }
            d[last_filled] = self.data_f_port.unwrap();
            last_filled += 1;
        }

        let mut enc_key = app_skey.0;
        if mac_cmds_len > 0 && has_fport_zero {
            enc_key = nwk_skey.0;
            payload_len = mac_cmds_len;
            if d.len() < last_filled + payload_len + MIC_LEN {
                return Err(Error::BufferTooShort);
            }
            d[last_filled..last_filled + payload_len].copy_from_slice(mac_cmds.as_ref());
        } else {
            if d.len() < last_filled + payload_len + MIC_LEN {
                return Err(Error::BufferTooShort);
            }
            d[last_filled..last_filled + payload_len].copy_from_slice(payload);
        };

        // Encrypt FRMPayload
        securityhelpers::encrypt_frm_data_payload(
            d,
            last_filled,
            last_filled + payload_len,
            self.fcnt,
            &factory.new_enc(&enc_key),
        );
        last_filled += payload_len;

        // MIC set
        let mic = securityhelpers::calculate_data_mic(
            &d[..last_filled],
            factory.new_mac(&nwk_skey.0),
            self.fcnt,
        );
        d[last_filled..last_filled + MIC_LEN].copy_from_slice(&mic.0);

        Ok(&d[..last_filled + MIC_LEN])
    }
}
