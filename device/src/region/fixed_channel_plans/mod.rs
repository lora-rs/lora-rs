use super::*;
use core::marker::PhantomData;
use lorawan::maccommands::ChannelMask;

// Compiler trick needed to assert array length at compile time
#[macro_export]
macro_rules! gen_assert {
    ($t:ident, $c:expr) => {{
        struct Check<const $t: usize>(usize);
        impl<const $t: usize> Check<$t> {
            const CHECK: () = assert!($c);
        }
        let _ = Check::<$t>::CHECK;
    }};
}

mod au915;
mod us915;

pub(crate) use au915::AU915;
pub(crate) use us915::US915;

seq_macro::seq!(
    N in 0..=71 {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Channel {
            #(
                _~N,
            )*
        }
    }
);

impl From<Channel> for u8 {
    fn from(value: Channel) -> Self {
        value as u8
    }
}

#[derive(Clone)]
struct PreferredJoinChannels {
    channel_list: heapless::Vec<u8, 16>,
    // Number representing the maximum number of retries allowed using the preferred channels list,
    max_retries: usize,
    // Decrementing number representing how many tries we have left with the specified list before
    // reverting to the default channel selection behavior.
    num_retries: usize,
}

impl PreferredJoinChannels {
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
struct AvailableSubbands(u16);

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

#[derive(Default, Clone)]
struct JoinChannels {
    preferred_channels: Option<PreferredJoinChannels>,
    // List of subbands that haven't already been tried.
    available_subbands: AvailableSubbands,
}

impl JoinChannels {
    /// Select a channel for a join attempt.
    ///
    /// ## RNG note:
    ///
    /// Uses 1 random number from the RNG.
    fn select_channel(&mut self, rng: &mut impl RngCore) -> u8 {
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

    fn set_preferred(&mut self, preferred: heapless::Vec<u8, 16>, max_retries: usize) -> Self {
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

#[derive(Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct FixedChannelPlan<const NUM_DR: usize, F: FixedChannelRegion<NUM_DR>> {
    last_tx_channel: u8,
    channel_mask: ChannelMask<9>,
    _fixed_channel_region: PhantomData<F>,
    join_channels: JoinChannels,
}

impl<const D: usize, F: FixedChannelRegion<D>> FixedChannelPlan<D, F> {
    pub fn set_preferred_join_channels(
        &mut self,
        preferred_channels: &[Channel],
        num_retries: usize,
    ) {
        let mut channel_vec = heapless::Vec::new();
        for chan in preferred_channels.iter().map(|c| *c as u8) {
            channel_vec.push(chan).unwrap();
        }
        self.join_channels.set_preferred(channel_vec, num_retries);
    }

    pub fn remove_preferred_join_channels(&mut self) {
        self.join_channels.preferred_channels = None;
    }

    pub fn set_125k_channels(&mut self, enabled: bool) {
        let mask = if enabled {
            0xFF
        } else {
            0x00
        };
        self.channel_mask.set_bank(0, mask);
        self.channel_mask.set_bank(1, mask);
        self.channel_mask.set_bank(2, mask);
        self.channel_mask.set_bank(3, mask);
        self.channel_mask.set_bank(4, mask);
        self.channel_mask.set_bank(5, mask);
        self.channel_mask.set_bank(6, mask);
        self.channel_mask.set_bank(7, mask);
    }
}

pub(crate) trait FixedChannelRegion<const NUM_DR: usize> {
    fn datarates() -> &'static [Option<Datarate>; NUM_DR];
    fn uplink_channels() -> &'static [u32; 72];
    fn downlink_channels() -> &'static [u32; 8];
    fn get_default_rx2() -> u32;
    fn get_rx_datarate(tx_datarate: DR, frame: &Frame, window: &Window) -> Datarate;
    fn get_dbm() -> i8;
}

impl<const D: usize, F: FixedChannelRegion<D>> RegionHandler for FixedChannelPlan<D, F> {
    fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) {
        // Reset the number of retries on the preferred channel list after a successful
        // join, in preparation for the next potential join attempt.
        if let Some(preferred) = &mut self.join_channels.preferred_channels {
            preferred.num_retries = preferred.max_retries;
        }

        if let Some(CfList::FixedChannel(channel_mask)) = join_accept.c_f_list() {
            self.channel_mask = channel_mask;
        }
    }

    fn handle_link_adr_channel_mask(
        &mut self,
        channel_mask_control: u8,
        channel_mask: ChannelMask<2>,
    ) {
        match channel_mask_control {
            0..=4 => {
                let base_index = channel_mask_control as usize * 2;
                self.channel_mask.set_bank(base_index, channel_mask.get_index(0));
                self.channel_mask.set_bank(base_index + 1, channel_mask.get_index(1));
            }
            5 => {
                let channel_mask: u16 =
                    channel_mask.get_index(0) as u16 | ((channel_mask.get_index(1) as u16) << 8);
                self.channel_mask.set_bank(0, ((channel_mask & 0b1) * 0xFF) as u8);
                self.channel_mask.set_bank(1, ((channel_mask & 0b10) * 0xFF) as u8);
                self.channel_mask.set_bank(2, ((channel_mask & 0b100) * 0xFF) as u8);
                self.channel_mask.set_bank(3, ((channel_mask & 0b1000) * 0xFF) as u8);
                self.channel_mask.set_bank(4, ((channel_mask & 0b10000) * 0xFF) as u8);
                self.channel_mask.set_bank(5, ((channel_mask & 0b100000) * 0xFF) as u8);
                self.channel_mask.set_bank(6, ((channel_mask & 0b1000000) * 0xFF) as u8);
                self.channel_mask.set_bank(7, ((channel_mask & 0b10000000) * 0xFF) as u8);
                self.channel_mask.set_bank(8, ((channel_mask & 0b100000000) * 0xFF) as u8);
            }
            6 => {
                self.set_125k_channels(true);
            }
            7 => {
                self.set_125k_channels(false);
            }
            _ => {
                //RFU
            }
        }
    }

    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        match frame {
            Frame::Join => {
                // For the join frame, the channel is selected using the following logic:
                //
                // * If favorite channels are specified, a channel from these will be selected
                //   at random until the
                // number of retries runs out (1 by default).
                // * Otherwise, a random channel will be selected from each group of 8 channels
                //   (including 500 kHz
                // channels) until every group of 8 has been tried, at which point every group
                // will be attempted again.
                //
                // As per RP002-1.0.4, all join attempts are made using DR0 for 125 kHz
                // channels, and DR4 for 500 kHz channels.
                // TODO: contradicting data rates for US915 vs AU915?
                let channel = self.join_channels.select_channel(rng);
                let dr = if channel < 64 {
                    DR::_0
                } else {
                    DR::_4
                };
                let datarate = F::datarates()[dr as usize].clone().unwrap();

                (datarate, channel as u32)
            }

            Frame::Data => {
                // For the data frame, the datarate impacts which channel sets we can choose
                // from. If the datarate bandwidth is 500 kHz, we must use
                // channels 64-71 Else, we must use 0-63
                let datarate = F::datarates()[datarate as usize].clone().unwrap();
                if datarate.bandwidth == Bandwidth::_500KHz {
                    let mut channel = (rng.next_u32() & 0b111) as u8;
                    // keep selecting a random channel until we find one that is enabled
                    while !self.channel_mask.is_enabled(channel.into()).unwrap() {
                        channel = (rng.next_u32() & 0b111) as u8;
                    }
                    self.last_tx_channel = channel;
                    (datarate, F::uplink_channels()[(64 + channel) as usize])
                } else {
                    let mut channel = (rng.next_u32() & 0b111111) as u8;
                    // keep selecting a random channel until we find one that is enabled
                    while !self.channel_mask.is_enabled(channel.into()).unwrap() {
                        channel = (rng.next_u32() & 0b111111) as u8;
                    }
                    self.last_tx_channel = channel;
                    (datarate, F::uplink_channels()[channel as usize])
                }
            }
        }
    }

    fn get_rx_frequency(&self, _frame: &Frame, window: &Window) -> u32 {
        let channel = self.last_tx_channel % 8;
        match window {
            Window::_1 => F::downlink_channels()[channel as usize],
            Window::_2 => F::get_default_rx2(),
        }
    }

    fn get_dbm(&self) -> i8 {
        F::get_dbm()
    }

    fn get_rx_datarate(&self, tx_datarate: DR, frame: &Frame, window: &Window) -> Datarate {
        F::get_rx_datarate(tx_datarate, frame, window)
    }
}

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
