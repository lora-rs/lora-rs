// Copyright (c) 2017-2020 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

//! Provides types and methods for creating LoRaWAN payloads.
//!
//! See [JoinAcceptCreator.new](struct.JoinAcceptCreator.html#method.new) for an example.

use super::keys;
use super::keys::CryptoFactory;
use super::maccommandcreator;
use super::maccommands::{mac_commands_len, SerializableMacCommand};
#[cfg(feature = "with-downlink")]
use super::maccommands::{DLSettings, Frequency};
use super::parser;
use super::securityhelpers;

#[cfg(feature = "default-crypto")]
use super::default_crypto::DefaultFactory;

#[cfg(feature = "with-downlink")]
use super::keys::Decrypter;

#[cfg(any(feature = "with-downlink", feature = "default-crypto"))]
use aes::cipher::generic_array::GenericArray;

#[cfg(feature = "default-crypto")]
use aes::cipher::generic_array::typenum::U256;

const PIGGYBACK_MAC_COMMANDS_MAX_LEN: usize = 15;

/// JoinAcceptCreator serves for creating binary representation of Physical
/// Payload of JoinAccept.
#[cfg(feature = "with-downlink")]
#[derive(Default)]
pub struct JoinAcceptCreator<D, F> {
    data: D,
    with_c_f_list: bool,
    encrypted: bool,
    factory: F,
}

#[cfg(feature = "with-downlink")]
impl<D: AsMut<[u8]>, F: CryptoFactory + Default> JoinAcceptCreator<D, F> {
    /// Creates a well initialized JoinAcceptCreator with specific data and crypto functions.
    ///
    /// TODO: Add more details & and example
    pub fn with_options<'a>(mut data: D, factory: F) -> Result<Self, &'a str> {
        // length verification will occur during building
        let d = data.as_mut();
        d[0] = 0x20;
        Ok(Self { data, with_c_f_list: false, encrypted: false, factory })
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
    ) -> Result<&mut Self, &str> {
        let ch_list = list.as_ref();
        if ch_list.len() > 5 {
            return Err("too many frequencies");
        }
        let d = self.data.as_mut();
        ch_list.iter().enumerate().for_each(|(i, fr)| {
            let v = fr.value() / 100;
            d[13 + i * 3] = (v & 0xff) as u8;
            d[14 + i * 3] = ((v >> 8) & 0xff) as u8;
            d[15 + i * 3] = ((v >> 16) & 0xff) as u8;
        });

        Ok(self)
    }

    /// Provides the binary representation of the encrypted join accept
    /// physical payload with the MIC set.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for encryption and setting the MIC.
    pub fn build(&mut self, key: &keys::AES128) -> Result<&[u8], &str> {
        let required_len = if self.with_c_f_list {
            33
        } else {
            17
        };
        if self.data.as_mut().len() < required_len {
            return Err("data slice is too short");
        }
        if !self.encrypted {
            self.encrypt_payload(key);
        }
        Ok(self.data.as_mut())
    }

    fn encrypt_payload(&mut self, key: &keys::AES128) {
        let d = if self.with_c_f_list {
            self.data.as_mut()
        } else {
            &mut self.data.as_mut()[..17]
        };
        set_mic(d, key, &self.factory);
        let aes_enc = self.factory.new_dec(key);
        for i in 0..(d.len() >> 4) {
            let start = (i << 4) + 1;
            let tmp = GenericArray::from_mut_slice(&mut d[start..(16 + start)]);
            aes_enc.decrypt_block(tmp);
        }
        self.encrypted = true;
    }
}

#[cfg(feature = "default-crypto,with-downlink")]
impl JoinAcceptCreator<[u8; 33], DefaultFactory> {
    /// Creates a well initialized JoinAcceptCreator.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut phy = lorawan::creator::JoinAcceptCreator::new();
    /// let key = lorawan::keys::AES128([1; 16]);
    /// let app_nonce_bytes = [1; 3];
    /// phy.set_app_nonce(&app_nonce_bytes);
    /// phy.set_net_id(&[1; 3]);
    /// phy.set_dev_addr(&[1; 4]);
    /// phy.set_dl_settings(2);
    /// phy.set_rx_delay(1);
    /// let mut freqs: Vec<lorawan::maccommands::Frequency> = Vec::new();
    /// freqs.push(lorawan::maccommands::Frequency::new(&[0x58, 0x6e, 0x84,]).unwrap()).unwrap();
    /// freqs.push(lorawan::maccommands::Frequency::new(&[0x88, 0x66, 0x84,]).unwrap()).unwrap();
    /// phy.set_c_f_list(freqs);
    /// let payload = phy.build(&key).unwrap();
    /// ```
    pub fn new() -> Self {
        let mut data = [0; 33];
        data[0] = 0x20;
        Self { data, with_c_f_list: false, encrypted: false, factory: DefaultFactory }
    }
}

fn set_mic<F: CryptoFactory>(data: &mut [u8], key: &keys::AES128, factory: &F) {
    let len = data.len();
    let mic = securityhelpers::calculate_mic(&data[..len - 4], factory.new_mac(key));

    data[len - 4..].copy_from_slice(&mic.0[..]);
}

/// JoinRequestCreator serves for creating binary representation of Physical
/// Payload of JoinRequest.
#[derive(Default)]
pub struct JoinRequestCreator<D, F> {
    data: D,
    factory: F,
}

impl<D: AsMut<[u8]>, F: CryptoFactory> JoinRequestCreator<D, F> {
    /// Creates a well initialized JoinRequestCreator with specific crypto functions.
    pub fn with_options<'a>(mut data: D, factory: F) -> Result<Self, &'a str> {
        let d = data.as_mut();
        if d.len() < 23 {
            return Err("data slice is too short");
        }
        d[0] = 0x00;
        Ok(Self { data, factory })
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
    pub fn set_dev_nonce<H: AsRef<[u8]>, T: Into<parser::DevNonce<H>>>(
        &mut self,
        dev_nonce: T,
    ) -> &mut Self {
        let converted = dev_nonce.into();
        self.data.as_mut()[17..19].copy_from_slice(converted.as_ref());

        self
    }

    /// Provides the binary representation of the JoinRequest physical payload
    /// with the MIC set.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for setting the MIC.
    pub fn build(&mut self, key: &keys::AES128) -> Result<&[u8], &str> {
        let d = self.data.as_mut();
        set_mic(d, key, &self.factory);
        Ok(d)
    }
}

/// DataPayloadCreator serves for creating binary representation of Physical
/// Payload of DataUp or DataDown messages.
///
/// # Example
///
/// ```
/// let mut phy = lorawan::creator::DataPayloadCreator::new();
/// let nwk_skey = lorawan::keys::AES128([2; 16]);
/// let app_skey = lorawan::keys::AES128([1; 16]);
/// phy.set_confirmed(true)
///     .set_uplink(true)
///     .set_f_port(42)
///     .set_dev_addr(&[4, 3, 2, 1])
///     .set_fctrl(&lorawan::parser::FCtrl::new(0x80, true)) // ADR: true, all others: false
///     .set_fcnt(76543);
/// phy.build(b"hello lora", &[], &nwk_skey, &app_skey).unwrap();
/// ```
#[derive(Default)]
pub struct DataPayloadCreator<D, F> {
    data: D,
    data_f_port: Option<u8>,
    fcnt: u32,
    factory: F,
}

impl<D: AsMut<[u8]>, F: CryptoFactory + Default> DataPayloadCreator<D, F> {
    /// Creates a well initialized DataPayloadCreator with specific crypto functions.
    ///
    /// By default the packet is unconfirmed data up packet.
    pub fn with_options<'a>(mut data: D, factory: F) -> Result<Self, &'a str> {
        let d = data.as_mut();
        if d.len() < 255 {
            return Err("data slice is too short");
        }
        d[0] = 0x40;
        Ok(DataPayloadCreator { data, data_f_port: None, fcnt: 0, factory })
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
    /// let mut phy = lorawan::creator::DataPayloadCreator::new();
    /// let mac_cmd1 = lorawan::maccommands::MacCommand::LinkCheckReq(
    ///     lorawan::maccommands::LinkCheckReqPayload());
    /// let mut mac_cmd2 = lorawan::maccommandcreator::LinkADRAnsCreator::new();
    /// mac_cmd2
    ///     .set_channel_mask_ack(true)
    ///     .set_data_rate_ack(false)
    ///     .set_tx_power_ack(true);
    /// let mut cmds: Vec<&dyn lorawan::maccommands::SerializableMacCommand> = Vec::new();
    /// cmds.push(&mac_cmd1);
    /// cmds.push(&mac_cmd2);
    /// let nwk_skey = lorawan::keys::AES128([2; 16]);
    /// let app_skey = lorawan::keys::AES128([1; 16]);
    /// phy.build(&[], &cmds, &nwk_skey, &app_skey).unwrap();
    /// ```
    pub fn build<'a>(
        &mut self,
        payload: &[u8],
        cmds: &[&dyn SerializableMacCommand],
        nwk_skey: &keys::AES128,
        app_skey: &keys::AES128,
    ) -> Result<&[u8], &'a str> {
        let d = self.data.as_mut();
        let mut last_filled = 8; // MHDR + FHDR without the FOpts
        let has_fport = self.data_f_port.is_some();
        let has_fport_zero = has_fport && self.data_f_port.unwrap() == 0;
        let mac_cmds_len = mac_commands_len(cmds);

        // Set MAC Commands
        if mac_cmds_len > PIGGYBACK_MAC_COMMANDS_MAX_LEN && !has_fport_zero {
            return Err("mac commands are too big for FOpts");
        }

        // Set FPort
        let mut payload_len = payload.len();

        if has_fport_zero && payload_len > 0 {
            return Err("mac commands in payload can not be send together with payload");
        }
        if !has_fport && payload_len > 0 {
            return Err("fport must be provided when there is FRMPayload");
        }
        // Set FOptsLen if present
        if !has_fport_zero && mac_cmds_len > 0 {
            d[5] |= mac_cmds_len as u8 & 0x0f;
            maccommandcreator::build_mac_commands(
                cmds,
                &mut d[last_filled..last_filled + mac_cmds_len],
            )
            .unwrap();
            last_filled += mac_cmds_len;
        }

        if has_fport {
            d[last_filled] = self.data_f_port.unwrap();
            last_filled += 1;
        }

        let mut enc_key = app_skey;
        if mac_cmds_len > 0 && has_fport_zero {
            enc_key = nwk_skey;
            payload_len = mac_cmds_len;
            maccommandcreator::build_mac_commands(
                cmds,
                &mut d[last_filled..last_filled + payload_len],
            )
            .unwrap();
        } else {
            d[last_filled..last_filled + payload_len].copy_from_slice(payload);
        };

        // Encrypt FRMPayload
        securityhelpers::encrypt_frm_data_payload(
            d,
            last_filled,
            last_filled + payload_len,
            self.fcnt,
            &self.factory.new_enc(enc_key),
        );

        // MIC set
        let mic = securityhelpers::calculate_data_mic(
            &d[..last_filled + payload_len],
            self.factory.new_mac(nwk_skey),
            self.fcnt,
        );
        d[last_filled + payload_len..last_filled + payload_len + 4].copy_from_slice(&mic.0);

        Ok(&d[..last_filled + payload_len + 4])
    }
}

#[cfg(feature = "default-crypto")]
impl DataPayloadCreator<GenericArray<u8, U256>, DefaultFactory> {
    /// Creates a well initialized DataPayloadCreator.
    ///
    /// By default the packet is unconfirmed data up packet.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut phy = lorawan::creator::DataPayloadCreator::new();
    /// let nwk_skey = lorawan::keys::AES128([2; 16]);
    /// let app_skey = lorawan::keys::AES128([1; 16]);
    /// let fctrl = lorawan::parser::FCtrl::new(0x80, true);
    /// phy.set_confirmed(false).
    ///     set_uplink(true).
    ///     set_f_port(1).
    ///     set_dev_addr(&[4, 3, 2, 1]).
    ///     set_fctrl(&fctrl). // ADR: true, all others: false
    ///     set_fcnt(1);
    /// let payload = phy.build(b"hello", &[], &nwk_skey, &app_skey).unwrap();
    /// ```
    pub fn new() -> Self {
        let mut data: GenericArray<u8, U256> = GenericArray::default();
        data[0] = 0x40;
        Self { data, data_f_port: None, fcnt: 0, factory: DefaultFactory }
    }
}
