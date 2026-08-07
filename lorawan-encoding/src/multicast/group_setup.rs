use crate::keys::{Crypto, McAppSKey, McKey, McNetSKey, NetworkCrypto, AES128};
use crate::multicast::McGroupSetupReqCreator;
use crate::{
    multicast::{McGroupSetupAnsCreator, McGroupSetupAnsPayload, McGroupSetupReqPayload},
    parser::McAddr,
};

#[derive(Debug)]
pub struct Session {
    multicast_addr: McAddr,
    mc_net_s_key: McNetSKey,
    mc_app_s_key: McAppSKey,
    pub fcnt_down: u32,
    max_fcnt_down: u32,
}

impl Session {
    pub fn new(
        multicast_addr: McAddr,
        mc_net_s_key: McNetSKey,
        mc_app_s_key: McAppSKey,
        fcnt_down: u32,
        max_fcnt_down: u32,
    ) -> Self {
        Self { multicast_addr, mc_net_s_key, mc_app_s_key, fcnt_down, max_fcnt_down }
    }
    pub fn multicast_addr(&self) -> McAddr {
        self.multicast_addr
    }
    pub fn mc_net_s_key(&self) -> McNetSKey {
        self.mc_net_s_key
    }
    pub fn mc_app_s_key(&self) -> McAppSKey {
        self.mc_app_s_key
    }

    pub fn max_fcnt_down(&self) -> u32 {
        self.max_fcnt_down
    }
}

impl McGroupSetupReqPayload<'_> {
    /*
     | McGroupIDHeader |  McAddr |   McKey_encrypted | minMcFCount | maxMcFCount |
     |       1         |    4    |         16        |   4         |     4       |
    */
    pub fn mc_group_id_header(&self) -> u8 {
        self.0[0] & 0b11
    }

    pub fn mc_addr(&self) -> McAddr {
        const OFFSET: usize = 1;
        const END: usize = OFFSET + McAddr::BYTE_LEN;
        McAddr::from_wire_bytes(self.0[OFFSET..END].try_into().unwrap())
    }

    pub(crate) fn mc_key_encrypted(&self) -> &[u8] {
        const OFFSET: usize = 1 + McAddr::BYTE_LEN;
        const END: usize = OFFSET + McKey::byte_len();
        &self.0[OFFSET..END]
    }

    /// Decrypts the McKey carried in the request.
    ///
    /// `crypto` must be bound to the McKEKey.
    pub fn mc_key_decrypted<C: Crypto>(&self, crypto: &C) -> McKey {
        let mut bytes: [u8; 16] = self.mc_key_encrypted().try_into().unwrap();
        crypto.encrypt_block(&mut bytes);
        McKey::from(bytes)
    }

    /// Derives the multicast session keys.
    ///
    /// `crypto` must be bound to the McKEKey. The `From<AES128>` bound is used to construct
    /// the crypto for the McKey decrypted from the request; implementations that cannot be
    /// built from a bare key can use [`Self::mc_key_decrypted`] and derive the session keys
    /// through [`McKey::derive_mc_app_s_key`] and [`McKey::derive_mc_net_s_key`] directly.
    pub fn derive_session_keys<C: Crypto + From<AES128>>(
        &self,
        crypto: &C,
    ) -> (McAppSKey, McNetSKey) {
        let mc_key = self.mc_key_decrypted(crypto);
        let mc_key_crypto = C::from(*mc_key.inner());
        let mc_addr = self.mc_addr();
        (
            McKey::derive_mc_app_s_key(&mc_key_crypto, &mc_addr),
            McKey::derive_mc_net_s_key(&mc_key_crypto, &mc_addr),
        )
    }

    /// Derives the multicast session and returns the assigned group ID.
    ///
    /// `crypto` must be bound to the McKEKey.
    pub fn derive_session<C: Crypto + From<AES128>>(&self, crypto: &C) -> (u8, Session) {
        let (mc_app_s_key, mc_net_s_key) = self.derive_session_keys(crypto);
        (
            self.mc_group_id_header(),
            Session {
                multicast_addr: self.mc_addr(),
                mc_net_s_key,
                mc_app_s_key,
                fcnt_down: self.min_mc_fcount(),
                max_fcnt_down: self.max_mc_fcount(),
            },
        )
    }

    /// `minMcFCount` is the next frame counter value of the multicast downlink to be sent by the
    /// server for this group
    pub fn min_mc_fcount(&self) -> u32 {
        const OFFSET: usize = 1 + McAddr::BYTE_LEN + McKey::byte_len();
        let bytes = &self.0[OFFSET..OFFSET + size_of::<u32>()];
        // tolerate unwrap here because we know the length is 4
        u32::from_le_bytes(bytes.try_into().unwrap())
    }

    /// `maxMcFCount` specifies the lifetime of this multicast group expressed as a maximum number
    /// of frames. The end-device will only accept a multicast downlink frame if the 32-bits frame
    /// counter value `minMcFCount ≤ McFCount < maxMcFCount`.
    pub fn max_mc_fcount(&self) -> u32 {
        const OFFSET: usize = 1 + McAddr::BYTE_LEN + McKey::byte_len() + size_of::<u32>();
        let bytes = &self.0[OFFSET..OFFSET + size_of::<u32>()];
        // tolerate unwrap here because we know the length is 4
        u32::from_le_bytes(bytes.try_into().unwrap())
    }
}

impl McGroupSetupAnsPayload<'_> {
    /*
     | McGroupIDHeader |
     |       1         |
    */
    pub fn mc_group_id_header(&self) -> u8 {
        self.0[0] & 0b11
    }
}

impl McGroupSetupAnsCreator {
    pub fn mc_group_id_header(&mut self, mc_group_id_header: u8) -> &mut Self {
        self.data[1] &= 0b1111_1100;
        self.data[1] |= mc_group_id_header & 0b11;
        self
    }
}

impl McGroupSetupReqCreator {
    pub fn mc_group_id_header(&mut self, mc_group_id_header: u8) -> &mut Self {
        const OFFSET: usize = 1;
        self.data[OFFSET] = mc_group_id_header;
        self
    }

    pub fn mc_addr(&mut self, addr: &McAddr) -> &mut Self {
        const OFFSET: usize = 2;
        const END: usize = OFFSET + 4;
        self.data[OFFSET..END].copy_from_slice(addr.as_wire_bytes());
        self
    }

    /// Encrypts the McKey into the request.
    ///
    /// `crypto` must be bound to the McKEKey.
    pub fn mc_key<C: NetworkCrypto>(&mut self, crypto: &C, mc_key: &McKey) -> &mut Self {
        const OFFSET: usize = 2 + McAddr::BYTE_LEN;
        const END: usize = OFFSET + McKey::byte_len();
        let block = &mut self.data[OFFSET..END];
        block.copy_from_slice(mc_key.as_ref());
        crypto.decrypt_block(block);
        self
    }

    pub fn min_mc_fcount(&mut self, fcount: u32) -> &mut Self {
        const OFFSET: usize = 2 + McAddr::BYTE_LEN + McKey::byte_len();

        const END: usize = OFFSET + 4;
        self.data[OFFSET..END].copy_from_slice(&fcount.to_le_bytes());
        self
    }

    pub fn max_mc_fcount(&mut self, fcount: u32) -> &mut Self {
        const OFFSET: usize = 2 + McAddr::BYTE_LEN + McKey::byte_len() + size_of::<u32>();
        self.data[OFFSET..OFFSET + size_of::<u32>()].copy_from_slice(&fcount.to_le_bytes());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_crypto::{DefaultCrypto, DefaultNetworkCrypto};
    use crate::keys::McKEKey;
    use crate::multicast::parse_downlink_multicast_commands;
    use crate::multicast::DownlinkRemoteSetup;

    #[test]
    fn roundtrip() {
        // Create a request with the encrypted key
        let mut req = McGroupSetupReqCreator::new();
        let mc_addr = McAddr::from_wire_bytes([52, 110, 29, 60]);
        let mc_key = McKey::from([0x44; 16]);
        let mcke_key = McKEKey::from([0x66; 16]);

        req.mc_group_id_header(0x01);
        req.mc_addr(&mc_addr);
        req.mc_key(&DefaultNetworkCrypto::new(mcke_key.inner()), &mc_key);
        req.min_mc_fcount(0x12345678);
        req.max_mc_fcount(0x87654321);
        let messages = req.build();
        let mut messages = parse_downlink_multicast_commands(messages).filter_map(Result::ok);
        let downlink_remote_setup = messages.next().unwrap();
        let mc_group_setup_req = match downlink_remote_setup {
            DownlinkRemoteSetup::McGroupSetupReq(mc_group_setup_req) => mc_group_setup_req,
            _ => panic!("Expected McGroupSetupReq"),
        };
        assert_eq!(mc_group_setup_req.mc_group_id_header(), 1);
        assert_eq!(mc_group_setup_req.mc_addr(), mc_addr);
        let decrypt_key =
            mc_group_setup_req.mc_key_decrypted(&DefaultCrypto::new(mcke_key.inner()));
        assert_eq!(decrypt_key.as_ref(), mc_key.as_ref());
        assert_eq!(mc_group_setup_req.min_mc_fcount(), 0x12345678);
        assert_eq!(mc_group_setup_req.max_mc_fcount(), 0x87654321);
    }
}
