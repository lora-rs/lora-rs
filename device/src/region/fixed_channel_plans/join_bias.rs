use super::*;

#[derive(Default, Clone)]
pub(crate) struct JoinChannels {
    pub(crate) preferred_channels: Option<PreferredJoinChannels>,
    // List of subbands that haven't already been tried.
    pub(crate) available_subbands: AvailableSubbands,
}

#[derive(Clone)]
pub(crate) struct PreferredJoinChannels {
    channel_list: heapless::Vec<u8, 16>,
    // Number representing the maximum number of retries allowed using the preferred channels list,
    max_retries: usize,
    // Decrementing number representing how many tries we have left with the specified list before
    // reverting to the default channel selection behavior.
    pub num_retries: usize,
}

impl PreferredJoinChannels {
    pub(crate) fn clear_num_retries(&mut self) {
        self.num_retries = self.max_retries
    }

    fn try_get_channel(&mut self, rng: &mut impl RngCore) -> Option<u8> {
        if self.num_retries > 0 {
            let random = rng.next_u32();
            self.num_retries -= 1;
            let len = self.channel_list.len();
            // TODO non-compliant because the channel might be the same as the previously
            // used channel?
            Some(self.channel_list[random as usize % len])
        } else {
            None
        }
    }
}

/// Bitflags containing subbands that haven't yet been tried for a join attempt
/// this round.
#[derive(Clone)]
pub(crate) struct AvailableSubbands(u16);

impl AvailableSubbands {
    const ALL_ENABLED: u16 = 0b111111111;

    fn new() -> Self {
        Self(Self::ALL_ENABLED)
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn pop_next(&mut self) -> Option<u8> {
        for bit in 0..=8 {
            if (self.0 >> bit) & 1 == 1 {
                self.0 &= !(1 << bit);
                return Some(bit);
            }
        }
        None
    }

    fn reset(&mut self) {
        self.0 = Self::ALL_ENABLED;
    }
}

impl Default for AvailableSubbands {
    fn default() -> Self {
        Self::new()
    }
}

impl JoinChannels {
    /// Select a channel for a join attempt.
    ///
    /// ## RNG note:
    ///
    /// Uses 1 random number from the RNG.
    pub(crate) fn select_channel(&mut self, rng: &mut impl RngCore) -> u8 {
        // Early-return preferred channel if possible
        if let Some(p) = self.preferred_channels.as_mut().and_then(|p| p.try_get_channel(rng)) {
            return p;
        }

        if self.available_subbands.is_empty() {
            self.available_subbands.reset();
        }

        // Unwrapping is ok because it should never be empty by this point
        let subband = self.available_subbands.pop_next().unwrap();
        8 * subband + (rng.next_u32() % 8) as u8
    }

    pub(crate) fn set_preferred(
        &mut self,
        preferred: heapless::Vec<u8, 16>,
        max_retries: usize,
    ) -> Self {
        Self {
            preferred_channels: Some(PreferredJoinChannels {
                channel_list: preferred,
                max_retries,
                num_retries: max_retries,
            }),
            ..Default::default()
        }
    }
}

// Compiler trick needed to assert array length at compile time. Used in macro below.
macro_rules! gen_assert {
    ($t:ident, $c:expr) => {{
        struct Check<const $t: usize>(usize);
        impl<const $t: usize> Check<$t> {
            const CHECK: () = assert!($c);
        }
        let _ = Check::<$t>::CHECK;
    }};
}

/// This macro implements public functions relating to a fixed plan region. This is preferred to a
/// trait implementation because the user does not have to worry about importing the trait to make
/// use of these functions.
macro_rules! impl_join_bias {
    ($region:ident) => {
        impl $region {
            pub fn new() -> Self {
                Self::default()
            }

            /// Specify a set of channels enabled
            /// for joining the network. You can specify up to 16 preferred channels.
            ///
            /// When `join` is called on a [`Configuration`] created using this
            /// region state, the network will be attempted to be joined only on the provided
            /// channel subset. This set of channels will be only be tried once; after which we will revert to trying to
            /// join with all channels enabled using a preset sequence. To specify a number of retries, use
            /// [`set_preferred_join_channels_and_noncompliant_retries`(Self::set_preferred_join_channels_and_noncompliant_retries).
            pub fn set_preferred_join_channels<const N: usize>(
                &mut self,
                preferred_channels: &[Channel; N],
            ) {
                self.set_preferred_join_channels_and_noncompliant_retries(preferred_channels, 1)
            }

            /// # ⚠️Warning⚠️
            ///
            /// This method is explicitly not compliant with the LoRaWAN spec when more than one
            /// try is attempted.
            ///
            /// This method is similar to `set_preferred_join_channels`, but allows you to
            /// specify the amount of times your preferred join channels should be attempted.
            ///
            /// It is recommended to set a low number (ie, < 10) of join retries using the
            /// preferred channels. The reason for this is if you *only* try to join
            /// with a channel bias, and the network is configured to use a
            /// strictly different set of channels than the ones you provide, the
            /// network will NEVER be joined.
            pub fn set_preferred_join_channels_and_noncompliant_retries<const N: usize>(
                &mut self,
                preferred_channels: &[Channel; N],
                num_retries: usize,
            ) {
                gen_assert!(N, N <= 16);
                self.0.set_preferred_join_channels(preferred_channels, num_retries)
            }

            pub fn remove_preferred_join_channels(&mut self) {
                self.0.remove_preferred_join_channels()
            }
        }
    };
}

impl_join_bias!(US915);

#[cfg(test)]
mod test {
    use super::*;
    // we do this impl From<u8> for Channel for testing purposes only
    // if we can avoid ever doing this in the real code, we can avoid the necessary error handling
    fn channel_from_u8(x: u8) -> Channel {
        unsafe { core::mem::transmute(x) }
    }

    #[test]
    fn test_u8_from_channel() {
        for i in 0..71 {
            let channel = channel_from_u8(i);
            // check a few by hand to make sure
            match i {
                0 => assert_eq!(channel, Channel::_0),
                1 => assert_eq!(channel, Channel::_1),
                71 => assert_eq!(channel, Channel::_71),
                // the rest can be verified using the From<Channel> for u8 impl
                _ => assert_eq!(i, u8::from(channel)),
            }
        }
    }
}
