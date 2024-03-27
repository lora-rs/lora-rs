use super::*;
use core::cmp::Ordering;

#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct JoinChannels {
    /// The maximum amount of times we attempt to join on the preferred subband.
    max_retries: usize,
    /// The amount of times we've currently attempted to join on the preferred subband.
    pub num_retries: usize,
    /// Preferred subband
    preferred_subband: Option<Subband>,
    /// Channels that have been attempted.
    pub(crate) available_channels: AvailableChannels,
    /// The channel used for the previous join request.
    pub(crate) previous_channel: u8,
}

impl JoinChannels {
    pub(crate) fn has_bias_and_not_exhausted(&self) -> bool {
        // there are remaining retries AND we have not yet been reset
        self.preferred_subband.is_some()
            && self.num_retries < self.max_retries
            && self.num_retries != 0
    }

    /// The first data channel will always be some random channel (possibly the same as previous)
    /// of the preferred subband. Returns None if there is no preferred subband.
    pub(crate) fn first_data_channel(&mut self, rng: &mut impl RngCore) -> Option<u8> {
        if self.preferred_subband.is_some() && self.num_retries != 0 {
            self.clear_join_bias();
            // determine which subband the successful join was sent on
            let sb = if self.previous_channel < 64 {
                self.previous_channel / 8
            } else {
                self.previous_channel % 8
            };
            // pick another channel on that subband
            Some((rng.next_u32() & 0b111) as u8 + (sb * 8))
        } else {
            None
        }
    }

    pub(crate) fn set_join_bias(&mut self, subband: Subband, max_retries: usize) {
        self.preferred_subband = Some(subband);
        self.max_retries = max_retries;
    }

    pub(crate) fn clear_join_bias(&mut self) {
        self.preferred_subband = None;
        self.max_retries = 0;
    }

    /// To be called after a join accept is received. Resets state for the next join attempt.
    pub(crate) fn reset(&mut self) {
        self.num_retries = 0;
        self.available_channels = AvailableChannels::default();
    }

    pub(crate) fn get_next_channel(&mut self, rng: &mut impl RngCore) -> u8 {
        match (self.preferred_subband, self.num_retries.cmp(&self.max_retries)) {
            (Some(sb), Ordering::Less) => {
                self.num_retries += 1;
                // pick a  random number 0-7 on the preferred subband
                // NB: we don't use 500 kHz channels
                let channel = (rng.next_u32() % 8) as u8 + ((sb as usize - 1) as u8 * 8);
                if self.num_retries == self.max_retries {
                    // this is our last try with our favorite subband, so will initialize the
                    // standard join logic with the channel we just tried. This will ensure
                    // standard and compliant behavior when num_retries is set to 1.
                    self.available_channels.previous = Some(channel);
                    self.available_channels.data.set_channel(channel.into(), false);
                }
                self.previous_channel = channel;
                channel
            }
            _ => {
                self.num_retries += 1;
                self.available_channels.get_next(rng)
            }
        }
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct AvailableChannels {
    data: ChannelMask<9>,
    previous: Option<u8>,
}

impl AvailableChannels {
    fn is_exhausted(&self) -> bool {
        // check if every underlying byte is entirely cleared to 0
        for byte in self.data.as_ref() {
            if *byte != 0 {
                return false;
            }
        }
        true
    }

    fn get_next(&mut self, rng: &mut impl RngCore) -> u8 {
        // this guarantees that there will be _some_ open channel available
        if self.is_exhausted() {
            self.reset();
        }

        let channel = self.get_next_channel_inner(rng);
        // mark the channel invalid for future selection
        self.data.set_channel(channel.into(), false);
        self.previous = Some(channel);
        channel
    }

    fn get_next_channel_inner(&mut self, rng: &mut impl RngCore) -> u8 {
        if let Some(previous) = self.previous {
            // choose the next one by possibly wrapping around
            let next = (previous + 8) % 72;
            // if the channel is valid, great!
            if self.data.is_enabled(next.into()).unwrap() {
                next
            } else {
                // We've wrapped around to our original random bank.
                // Randomly select a new channel on the original bank.
                // NB: there shall always be something because this will be the first
                // bank to get exhausted and the caller of this function will reset
                // when the last one is exhausted.
                let bank = next / 8;
                let mut entropy = rng.next_u32();
                let mut channel = (entropy & 0b111) as u8 + bank * 8;
                let mut entropy_used = 1;
                loop {
                    if self.data.is_enabled(channel.into()).unwrap() {
                        return channel;
                    } else {
                        // we've used 30 of the 32 bits of entropy. reset the byte
                        if entropy_used == 10 {
                            entropy = rng.next_u32();
                            entropy_used = 0;
                        }
                        entropy >>= 3;
                        entropy_used += 1;
                        channel = (entropy & 0b111) as u8 + bank * 8;
                    }
                }
            }
        } else {
            // pick a completely random channel on the bottom 64
            // NB: all channels are currently valid
            (rng.next_u32() as u8) & 0b111111
        }
    }

    fn reset(&mut self) {
        self.data = ChannelMask::default();
        self.previous = None;
    }
}

/// This macro implements public functions relating to a fixed plan region. This is preferred to a
/// trait implementation because the user does not have to worry about importing the trait to make
/// use of these functions.
macro_rules! impl_join_bias {
    ($region:ident) => {
        impl $region {
            /// Create this struct directly if you want to specify a subband on which to bias the join process.
            pub fn new() -> Self {
                Self::default()
            }

            /// Specify a preferred subband when joining the network. Only the first join attempt
            /// will occur on this subband. After that, each bank will be attempted sequentially
            /// as described in the US915/AU915 regional specifications.
            pub fn set_join_bias(&mut self, subband: Subband) {
                self.0.join_channels.set_join_bias(subband, 1)
            }

            /// # ⚠️Warning⚠️
            ///
            /// This method is explicitly not compliant with the LoRaWAN spec when more than one
            /// try is attempted.
            ///
            /// This method is similar to `set_join_bias`, but allows you to specify a potentially
            /// non-compliant amount of times your preferred join subband should be attempted.
            ///
            /// It is recommended to set a low number (ie, < 10) of join retries using the
            /// preferred subband. The reason for this is if you *only* try to join
            /// with a channel bias, and the network is configured to use a
            /// strictly different set of channels than the ones you provide, the
            /// network will NEVER be joined.
            pub fn set_join_bias_and_noncompliant_retries(
                &mut self,
                subband: Subband,
                max_retries: usize,
            ) {
                self.0.join_channels.set_join_bias(subband, max_retries)
            }

            pub fn clear_join_bias(&mut self) {
                self.0.join_channels.clear_join_bias()
            }
        }
    };
}

#[cfg(feature = "region-au915")]
impl_join_bias!(AU915);
#[cfg(feature = "region-us915")]
impl_join_bias!(US915);

#[cfg(test)]
mod test {
    use super::*;
    use crate::mac::Response;
    use crate::{
        mac::{Mac, SendData},
        test_util::{get_key, handle_join_request, Uplink},
        AppEui, AppKey, DevEui, NetworkCredentials,
    };
    use heapless::Vec;
    use lorawan::default_crypto::DefaultFactory;

    #[test]
    fn test_join_channels_standard() {
        let mut rng = rand_core::OsRng;
        // run the test a bunch of times due to the rng
        for _ in 0..100 {
            let mut join_channels = JoinChannels::default();
            let first_channel = join_channels.get_next_channel(&mut rng);
            // the first channel is always in the bottom 64
            assert!(first_channel < 64);
            let next_channel = join_channels.get_next_channel(&mut rng);
            // the next channel is always incremented by 8, since we always have
            // the fat bank (channels 64-71)
            assert_eq!(next_channel, first_channel + 8);
            // we generate 6 more channels
            for _ in 0..7 {
                let c = join_channels.get_next_channel(&mut rng);
                assert!(c < 72);
            }
            // after 8 tries, we should be back at the original bank but on a different channel
            let ninth_channel = join_channels.get_next_channel(&mut rng);
            assert_eq!(ninth_channel / 8, first_channel / 8);
            assert_ne!(ninth_channel, first_channel);
        }
    }

    #[test]
    fn test_join_channels_standard_exhausted() {
        let mut rng = rand_core::OsRng;

        let mut join_channels = JoinChannels::default();
        let first_channel = join_channels.get_next_channel(&mut rng);
        // the first channel is always in the bottom 64
        assert!(first_channel < 64);
        let next_channel = join_channels.get_next_channel(&mut rng);
        // the next channel is always incremented by 8, since we always have
        // the fat bank (channels 64-71)
        assert_eq!(next_channel, first_channel + 8);
        // we generate 6000
        for _ in 0..6000 {
            let c = join_channels.get_next_channel(&mut rng);
            assert!(c < 72);
        }
    }

    #[test]
    fn test_join_channels_biased() {
        let mut rng = rand_core::OsRng;
        // run the test a bunch of times due to the rng
        for _ in 0..100 {
            let mut join_channels = JoinChannels::default();
            join_channels.set_join_bias(Subband::_2, 1);
            let first_channel = join_channels.get_next_channel(&mut rng);
            // the first is on subband 2
            assert!(first_channel > 7);
            assert!(first_channel < 16);
            let next_channel = join_channels.get_next_channel(&mut rng);
            // the next channel is always incremented by 8, since we always have
            // the fat bank (channels 64-71)
            assert_eq!(next_channel, first_channel + 8);
            // we generate 6 more channels
            for _ in 0..7 {
                let c = join_channels.get_next_channel(&mut rng);
                assert!(c < 72);
            }
            // after 8 tries, we should be back at the biased bank but on a different channel
            let ninth_channel = join_channels.get_next_channel(&mut rng);
            assert_eq!(ninth_channel / 8, first_channel / 8);
            assert_ne!(ninth_channel, first_channel);
        }
    }

    #[test]
    fn test_full_mac_compliant_bias() {
        let mut us915 = US915::new();
        us915.set_join_bias(Subband::_2);
        let mut mac = Mac::new(us915.into(), 21, 2);

        let mut buf: RadioBuffer<255> = RadioBuffer::new();
        let (tx_config, _len) = mac.join_otaa::<DefaultFactory, _, 255>(
            &mut rand::rngs::OsRng,
            NetworkCredentials::new(
                AppEui::from([0x0; 8]),
                DevEui::from([0x0; 8]),
                AppKey::from(get_key()),
            ),
            &mut buf,
        );
        // Confirm that the join request occurs on our subband
        assert!(
            tx_config.rf.frequency >= 903_900_000,
            "Unexpected frequency: {} is below 903.9 MHz!",
            tx_config.rf.frequency
        );
        assert!(
            tx_config.rf.frequency <= 905_300_000,
            "Unexpected frequency: {} is above 905.3 MHz!",
            tx_config.rf.frequency
        );
        let mut downlinks: Vec<_, 3> = Vec::new();
        let mut data = std::vec::Vec::new();
        data.extend_from_slice(buf.as_ref_for_read());
        let uplink = Uplink::new(buf.as_ref_for_read(), tx_config).unwrap();

        let mut rx_buf = [0; 255];
        let len = handle_join_request::<0>(Some(uplink), tx_config.rf, &mut rx_buf);
        buf.clear();
        buf.extend_from_slice(&rx_buf[..len]).unwrap();
        let response = mac.handle_rx::<DefaultFactory, 255, 3>(&mut buf, &mut downlinks);
        if let Response::JoinSuccess = response {
        } else {
            panic!("Did not receive join success");
        }
        let (tx_config, _len) = mac
            .send::<DefaultFactory, _, 255>(
                &mut rand::rngs::OsRng,
                &mut buf,
                &SendData { fport: 1, data: &[0x0; 1], confirmed: false },
            )
            .unwrap();
        // Confirm that the first data frame occurs on our subband
        assert!(
            tx_config.rf.frequency >= 903_900_000,
            "Unexpected frequency: {} is below 903.9 MHz!",
            tx_config.rf.frequency
        );
        assert!(
            tx_config.rf.frequency <= 905_300_000,
            "Unexpected frequency: {} is above 905.3 MHz!",
            tx_config.rf.frequency
        );
    }

    #[test]
    fn test_full_mac_non_compliant_bias() {
        let mut us915 = US915::new();
        us915.set_join_bias_and_noncompliant_retries(Subband::_2, 8);
        let mut mac = Mac::new(us915.into(), 21, 2);

        let mut buf: RadioBuffer<255> = RadioBuffer::new();
        let (tx_config, _len) = mac.join_otaa::<DefaultFactory, _, 255>(
            &mut rand::rngs::OsRng,
            NetworkCredentials::new(
                AppEui::from([0x0; 8]),
                DevEui::from([0x0; 8]),
                AppKey::from(get_key()),
            ),
            &mut buf,
        );
        // Confirm that the join request occurs on our subband
        assert!(
            tx_config.rf.frequency >= 903_900_000,
            "Unexpected frequency: {} is below 903.9 MHz!",
            tx_config.rf.frequency
        );
        assert!(
            tx_config.rf.frequency <= 905_300_000,
            "Unexpected frequency: {} is above 905.3 MHz!",
            tx_config.rf.frequency
        );
        let mut downlinks: Vec<_, 3> = Vec::new();
        let mut data = std::vec::Vec::new();
        data.extend_from_slice(buf.as_ref_for_read());
        let uplink = Uplink::new(buf.as_ref_for_read(), tx_config).unwrap();

        let mut rx_buf = [0; 255];
        let len = handle_join_request::<0>(Some(uplink), tx_config.rf, &mut rx_buf);
        buf.clear();
        buf.extend_from_slice(&rx_buf[..len]).unwrap();
        let response = mac.handle_rx::<DefaultFactory, 255, 3>(&mut buf, &mut downlinks);
        if let Response::JoinSuccess = response {
        } else {
            panic!("Did not receive JoinSuccess")
        }
        for _ in 0..8 {
            let (tx_config, _len) = mac
                .send::<DefaultFactory, _, 255>(
                    &mut rand::rngs::OsRng,
                    &mut buf,
                    &SendData { fport: 1, data: &[0x0; 1], confirmed: false },
                )
                .unwrap();
            // Confirm that the first data frame occurs on our subband
            assert!(
                tx_config.rf.frequency >= 903_900_000,
                "Unexpected frequency: {} is below 903.9 MHz!",
                tx_config.rf.frequency
            );
            assert!(
                tx_config.rf.frequency <= 905_300_000,
                "Unexpected frequency: {} is above 905.3 MHz!",
                tx_config.rf.frequency
            );
            mac.rx2_complete();
        }
    }
}
