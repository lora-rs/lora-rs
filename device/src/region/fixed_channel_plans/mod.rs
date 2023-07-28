use super::*;
use core::marker::PhantomData;
use lorawan::maccommands::ChannelMask;

mod au915;
mod us915;

pub(crate) use au915::AU915;
pub(crate) use us915::US915;

#[derive(Clone)]
struct PreferredJoinChannels {
    channel_list: heapless::Vec<u8, 16>,
    // Decrementing number representing how many tries we have left with the specified list before
    // reverting to the default channel selection behavior.
    num_retries: usize,
}

#[derive(Default, Clone)]
struct JoinChannels {
    preferred_channels: Option<PreferredJoinChannels>,
    // List of subbands that haven't already been tried.
    available_subbands: heapless::Vec<u8, 9>,
}

impl JoinChannels {
    fn select_channel(&mut self, rng: &mut impl RngCore) -> u8 {
        let random = rng.next_u32();

        if let Some(fav) = &mut self.preferred_channels {
            if fav.num_retries > 0 {
                fav.num_retries -= 1;
                let len = fav.channel_list.len();
                // TODO non-compliant because the channel might be the same as the previously
                // used channel?
                return fav.channel_list[random as usize % len];
            }
        }

        if self.available_subbands.is_empty() {
            for i in (0..=8).rev() {
                self.available_subbands.push(i).unwrap();
            }
        }

        let subband = self.available_subbands.pop().unwrap();
        8 * subband + (rng.next_u32() % 8) as u8
    }

    fn set_preferred(&mut self, preferred: heapless::Vec<u8, 16>, num_retries: usize) -> Self {
        Self {
            preferred_channels: Some(PreferredJoinChannels {
                channel_list: preferred,
                num_retries,
            }),
            ..Default::default()
        }
    }
}

#[derive(Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct FixedChannelPlan<const D: usize, F: FixedChannelRegion<D>> {
    last_tx_channel: u8,
    channel_mask: ChannelMask<9>,
    _fixed_channel_region: PhantomData<F>,
    join_channels: JoinChannels,
}

impl<const D: usize, F: FixedChannelRegion<D>> FixedChannelPlan<D, F> {
    pub fn set_preferred_join_channels(&mut self, preferred_channels: &[u8], num_retries: usize) {
        self.join_channels
            .set_preferred(heapless::Vec::from_slice(preferred_channels).unwrap(), num_retries);
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

pub(crate) trait FixedChannelRegion<const D: usize> {
    fn datarates() -> &'static [Option<Datarate>; D];
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
