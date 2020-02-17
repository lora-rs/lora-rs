// Copyright (c) 2017,2018 Ivaylo Petrov
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//
// author: Ivaylo Petrov <ivajloip@gmail.com>

use super::keys;
use super::maccommandcreator;
use super::maccommands;
use super::parser;
use super::securityhelpers;

use aes::block_cipher_trait::generic_array::GenericArray;
use aes::block_cipher_trait::BlockCipher;
use aes::Aes128;

const PIGGYBACK_MAC_COMMANDS_MAX_LEN: usize = 15;

/// JoinAcceptCreator serves for creating binary representation of Physical
/// Payload of JoinAccept.
#[derive(Default)]
pub struct JoinAcceptCreator {
    data: Vec<u8>,
    encrypted: bool,
}

impl JoinAcceptCreator {
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
    /// phy.set_c_f_list(vec![lorawan::maccommands::Frequency::new(&[0x58, 0x6e, 0x84,]).unwrap(),
    ///      lorawan::maccommands::Frequency::new(&[0x88, 0x66, 0x84,]).unwrap()]);
    /// let payload = phy.build(&key).unwrap();
    /// ```
    pub fn new() -> JoinAcceptCreator {
        let mut data = vec![0; 17];
        data[0] = 0x20;
        JoinAcceptCreator {
            data,
            ..Default::default()
        }
    }

    /// Sets the AppNonce of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * app_nonce - instance of lorawan::parser::AppNonce or anything that can
    ///   be converted into it.
    pub fn set_app_nonce<'a, T: Into<parser::AppNonce<'a>>>(
        &mut self,
        app_nonce: T,
    ) -> &mut JoinAcceptCreator {
        let converted = app_nonce.into();
        self.data[1..4].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the network ID of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * net_id - instance of lorawan::parser::NwkAddr or anything that can
    ///   be converted into it.
    pub fn set_net_id<'a, T: Into<parser::NwkAddr<'a>>>(&mut self, net_id: T) -> &mut JoinAcceptCreator {
        let converted = net_id.into();
        self.data[4..7].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the device address of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_addr - instance of lorawan::parser::DevAddr or anything that can
    ///   be converted into it.
    pub fn set_dev_addr<'a, T: Into<parser::DevAddr<'a>>>(
        &mut self,
        dev_addr: T,
    ) -> &mut JoinAcceptCreator {
        let converted = dev_addr.into();
        self.data[7..11].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the DLSettings of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * dl_settings - instance of lorawan::maccommands::DLSettings or anything
    ///   that can be converted into it.
    pub fn set_dl_settings<T: Into<maccommands::DLSettings>>(
        &mut self,
        dl_settings: T,
    ) -> &mut JoinAcceptCreator {
        let converted = dl_settings.into();
        self.data[11] = converted.raw_value();

        self
    }

    /// Sets the RX delay of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * rx_delay - the rx delay for the first receive window.
    pub fn set_rx_delay(&mut self, rx_delay: u8) -> &mut JoinAcceptCreator {
        self.data[12] = rx_delay;

        self
    }

    /// Sets the CFList of the JoinAccept to the provided value.
    ///
    /// # Argument
    ///
    /// * ch_list - list of Frequences to be sent to the device.
    pub fn set_c_f_list(
        &mut self,
        ch_list: Vec<maccommands::Frequency>,
    ) -> Result<&mut JoinAcceptCreator, &str> {
        if ch_list.len() > 5 {
            return Err("too many frequences");
        }
        if self.data.len() < 33 {
            self.data.resize(33, 0);
        }
        ch_list.iter().enumerate().for_each(|(i, fr)| {
            let v = fr.value() / 100;
            self.data[13 + i * 3] = (v & 0xff) as u8;
            self.data[14 + i * 3] = ((v >> 8) & 0xff) as u8;
            self.data[15 + i * 3] = ((v >> 16) & 0xff) as u8;
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
        if !self.encrypted {
            self.encrypt_payload(key);
        }
        Ok(&self.data[..])
    }

    fn encrypt_payload(&mut self, key: &keys::AES128) {
        set_mic(&mut self.data[..], key);
        let aes_enc = Aes128::new(GenericArray::from_slice(&key.0[..]));
        for i in 0..(self.data.len() >> 4) {
            let start = (i << 4) + 1;
            let mut tmp = GenericArray::from_mut_slice(&mut self.data[start..(16 + start)]);
            aes_enc.decrypt_block(&mut tmp);
        }
        self.encrypted = true;
    }
}

fn set_mic(data: &mut [u8], key: &keys::AES128) {
    let len = data.len();
    let mic = securityhelpers::calculate_mic(&data[..len - 4], key);

    data[len - 4..].copy_from_slice(&mic.0[..]);
}

/// JoinRequestCreator serves for creating binary representation of Physical
/// Payload of JoinRequest.
#[derive(Default)]
pub struct JoinRequestCreator {
    data: Vec<u8>,
}

impl JoinRequestCreator {
    /// Creates a well initialized JoinRequestCreator.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut phy = lorawan::creator::JoinRequestCreator::new();
    /// let key = lorawan::keys::AES128([7; 16]);
    /// phy.set_app_eui(&[1; 8]);
    /// phy.set_dev_eui(&[2; 8]);
    /// phy.set_dev_nonce(&[3; 2]);
    /// let payload = phy.build(&key).unwrap();
    /// ```
    pub fn new() -> JoinRequestCreator {
        let mut data = vec![0; 23];
        data[0] = 0x00;
        JoinRequestCreator { data }
    }

    /// Sets the application EUI of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * app_eui - instance of lorawan::parser::EUI64 or anything that can
    ///   be converted into it.
    pub fn set_app_eui<'a, T: Into<parser::EUI64<'a>>>(
        &mut self,
        app_eui: T,
    ) -> &mut JoinRequestCreator {
        let converted = app_eui.into();
        self.data[1..9].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the device EUI of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_eui - instance of lorawan::parser::EUI64 or anything that can
    ///   be converted into it.
    pub fn set_dev_eui<'a, T: Into<parser::EUI64<'a>>>(
        &mut self,
        dev_eui: T,
    ) -> &mut JoinRequestCreator {
        let converted = dev_eui.into();
        self.data[9..17].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the device nonce of the JoinRequest to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_nonce - instance of lorawan::parser::DevNonce or anything that can
    ///   be converted into it.
    pub fn set_dev_nonce<'a, T: Into<parser::DevNonce<'a>>>(
        &mut self,
        dev_nonce: T,
    ) -> &mut JoinRequestCreator {
        let converted = dev_nonce.into();
        self.data[17..19].copy_from_slice(converted.as_ref());

        self
    }

    /// Provides the binary representation of the JoinRequest physical payload
    /// with the MIC set.
    ///
    /// # Argument
    ///
    /// * key - the key to be used for setting the MIC.
    pub fn build(&mut self, key: &keys::AES128) -> Result<&[u8], &str> {
        set_mic(&mut self.data[..], key);
        Ok(&self.data[..])
    }
}

/// DataPayloadCreator serves for creating binary representation of Physical
/// Payload of DataUp or DataDown messages.
#[derive(Default)]
pub struct DataPayloadCreator {
    data: Vec<u8>,
    mac_commands_bytes: Vec<u8>,
    encrypt_mac_commands: bool,
    data_f_port: Option<u8>,
    fcnt: u32,
}

impl DataPayloadCreator {
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
    /// phy.set_confirmed(false);
    /// phy.set_uplink(true);
    /// phy.set_f_port(1);
    /// phy.set_dev_addr(&[4, 3, 2, 1]);
    /// phy.set_fctrl(&fctrl); // ADR: true, all others: false
    /// phy.set_fcnt(1);
    /// let payload = phy.build(b"hello", &nwk_skey, &app_skey).unwrap();
    /// ```
    pub fn new() -> DataPayloadCreator {
        let mut data = vec![0; 12];
        data[0] = 0x40;
        DataPayloadCreator {
            data,
            ..Default::default()
        }
    }

    /// Sets whether the packet is uplink or downlink.
    ///
    /// # Argument
    ///
    /// * uplink - whether the packet is uplink or downlink.
    pub fn set_uplink(&mut self, uplink: bool) -> &mut DataPayloadCreator {
        if uplink {
            self.data[0] &= 0xdf;
        } else {
            self.data[0] |= 0x20;
        }
        self
    }

    /// Sets whether the packet is confirmed or unconfirmed.
    ///
    /// # Argument
    ///
    /// * confirmed - whether the packet is confirmed or unconfirmed.
    pub fn set_confirmed(&mut self, confirmed: bool) -> &mut DataPayloadCreator {
        if confirmed {
            self.data[0] &= 0xbf;
            self.data[0] |= 0x80;
        } else {
            self.data[0] &= 0x7f;
            self.data[0] |= 0x40;
        }

        self
    }

    /// Sets the device address of the DataPayload to the provided value.
    ///
    /// # Argument
    ///
    /// * dev_addr - instance of lorawan::parser::DevAddr or anything that can
    ///   be converted into it.
    pub fn set_dev_addr<'a, T: Into<parser::DevAddr<'a>>>(
        &mut self,
        dev_addr: T,
    ) -> &mut DataPayloadCreator {
        let converted = dev_addr.into();
        self.data[1..5].copy_from_slice(converted.as_ref());

        self
    }

    /// Sets the FCtrl header of the DataPayload packet to the specified value.
    ///
    /// # Argument
    ///
    /// * fctrl - the FCtrl to be set.
    pub fn set_fctrl(&mut self, fctrl: &parser::FCtrl) -> &mut DataPayloadCreator {
        self.data[5] = fctrl.raw_value();
        self
    }

    /// Sets the FCnt header of the DataPayload packet to the specified value.
    ///
    /// NOTE: In the packet header the value will be truncated to u16.
    ///
    /// # Argument
    ///
    /// * fctrl - the FCtrl to be set.
    pub fn set_fcnt(&mut self, fcnt: u32) -> &mut DataPayloadCreator {
        self.fcnt = fcnt;
        self.data[6] = (fcnt & (0xff as u32)) as u8;
        self.data[7] = (fcnt >> 8) as u8;

        self
    }

    /// Sets the FPort header of the DataPayload packet to the specified value.
    ///
    /// If f_port == 0, automatically sets `encrypt_mac_commands` to `true`.
    ///
    /// # Argument
    ///
    /// * f_port - the FPort to be set.
    pub fn set_f_port(&mut self, f_port: u8) -> &mut DataPayloadCreator {
        if f_port == 0 {
            self.encrypt_mac_commands = true;
        }
        self.data_f_port = Some(f_port);

        self
    }

    /// Sets the mac commands to be used.
    ///
    /// Based on f_port value and value of encrypt_mac_commands, the mac commands will be sent
    /// either as payload or piggybacked.
    ///
    /// # Examples:
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
    /// let cmds: Vec<&lorawan::maccommands::SerializableMacCommand> = vec![&mac_cmd1, &mac_cmd2];
    /// phy.set_mac_commands(cmds);
    /// ```
    pub fn set_mac_commands<'a>(
        &'a mut self,
        cmds: Vec<&dyn maccommands::SerializableMacCommand>,
    ) -> &mut DataPayloadCreator {
        self.mac_commands_bytes = maccommandcreator::build_mac_commands(&cmds[..]);

        self
    }

    /// Whether the mac commands should be encrypted.
    ///
    /// NOTE: Setting the f_port to 0 implicitly sets the mac commands to be encrypted.
    pub fn set_encrypt_mac_commands(&mut self, encrypt: bool) -> &mut DataPayloadCreator {
        self.encrypt_mac_commands = encrypt;

        self
    }

    /// Whether a set of mac commands can be piggybacked.
    pub fn can_piggyback(cmds: Vec<&dyn maccommands::SerializableMacCommand>) -> bool {
        maccommands::mac_commands_len(&cmds[..]) <= PIGGYBACK_MAC_COMMANDS_MAX_LEN
    }

    /// Provides the binary representation of the DataPayload physical payload
    /// with the MIC set and payload encrypted.
    ///
    /// # Argument
    ///
    /// * payload - the FRMPayload (application) to be sent.
    /// * nwk_skey - the key to be used for setting the MIC and possibly for
    ///   MAC command encryption.
    /// * app_skey - the key to be used for payload encryption if fport not 0,
    ///   otherwise nwk_skey is only used.
    pub fn build(
        &mut self,
        payload: &[u8],
        nwk_skey: &keys::AES128,
        app_skey: &keys::AES128,
    ) -> Result<&[u8], &str> {
        let mut last_filled = 8; // MHDR + FHDR without the FOpts
        let has_fport = self.data_f_port.is_some();
        let has_fport_zero = has_fport && self.data_f_port.unwrap() == 0;

        // Set MAC Commands
        if self.mac_commands_bytes.len() > PIGGYBACK_MAC_COMMANDS_MAX_LEN && has_fport
            && self.data_f_port.unwrap() != 0
        {
            return Err("mac commands are too big for FOpts");
        }
        if self.encrypt_mac_commands && has_fport && !has_fport_zero {
            return Err("mac commands in payload require FPort == 0");
        }
        if !self.encrypt_mac_commands && has_fport_zero {
            return Err("mac commands have to be encrypted when FPort is 0");
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
        if !self.encrypt_mac_commands && !self.mac_commands_bytes.is_empty() {
            let mac_cmds_len = self.mac_commands_bytes.len();
            self.data[5] |= mac_cmds_len as u8 & 0x0f;
            self.data[last_filled..last_filled + mac_cmds_len]
                .copy_from_slice(&self.mac_commands_bytes[..]);
            last_filled += mac_cmds_len;
        }
        if has_fport {
            self.data[last_filled] = self.data_f_port.unwrap();
            last_filled += 1;
        }

        // Encrypt FRMPayload
        let encrypted_payload = if has_fport_zero {
            payload_len = self.mac_commands_bytes.len();
            securityhelpers::encrypt_frm_data_payload(
                &self.data[..],
                &self.mac_commands_bytes[..],
                self.fcnt,
                nwk_skey,
            )?
        } else {
            securityhelpers::encrypt_frm_data_payload(
                &self.data[..],
                payload,
                self.fcnt,
                app_skey,
            )?
        };

        // Set payload if possible, otherwise return error
        let additional_bytes_needed = last_filled + payload_len + 4 - self.data.len();
        if additional_bytes_needed > 0 {
            // we don't have enough length to accomodate all the bytes
            self.data.reserve_exact(additional_bytes_needed);
            unsafe {
                self.data.set_len(last_filled + payload_len + 4);
            }
        }
        if payload_len > 0 {
            self.data[last_filled..last_filled + payload_len]
                .copy_from_slice(&encrypted_payload[..]);
        }

        // MIC set
        let len = self.data.len();
        let mic = securityhelpers::calculate_data_mic(&self.data[..len - 4], nwk_skey, self.fcnt);
        self.data[len - 4..].copy_from_slice(&mic.0[..]);

        Ok(&self.data[..])
    }
}
