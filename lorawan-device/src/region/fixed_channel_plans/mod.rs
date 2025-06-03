use super::*;
use core::marker::PhantomData;
use lorawan::maccommands::ChannelMask;

mod join_channels;
use join_channels::JoinChannels;

#[cfg(feature = "region-au915")]
mod au915;
#[cfg(feature = "region-us915")]
mod us915;

#[cfg(feature = "region-au915")]
pub use au915::AU915;
#[cfg(feature = "region-us915")]
pub use us915::US915;

/// Subband definitions used to bias the join process for regions with fixed channel plans (ie: [`US915`], [`AU915`]).
///
/// Each Subband holds 8 channels. eg: subband 1 contains: channels 0-7, subband 2: channels 8-15, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(usize)]
pub enum Subband {
    _1 = 1,
    _2 = 2,
    _3 = 3,
    _4 = 4,
    _5 = 5,
    _6 = 6,
    _7 = 7,
    _8 = 8,
}

impl From<Subband> for usize {
    fn from(value: Subband) -> Self {
        value as usize
    }
}

#[derive(Clone)]
pub(crate) struct FixedChannelPlan<F: FixedChannelRegion> {
    last_tx_channel: u8,
    channel_mask: ChannelMask<9>,
    _fixed_channel_region: PhantomData<F>,
    join_channels: JoinChannels,

    frequency_valid: fn(u32) -> bool,
}

impl<F: FixedChannelRegion> FixedChannelPlan<F> {
    pub fn new(freq_fn: fn(u32) -> bool) -> Self {
        Self {
            last_tx_channel: Default::default(),
            channel_mask: Default::default(),
            _fixed_channel_region: Default::default(),
            join_channels: Default::default(),
            frequency_valid: freq_fn,
        }
    }

    pub fn set_125k_channels(
        &self,
        channel_mask: &mut ChannelMask<9>,
        enabled: bool,
        extra_mask: ChannelMask<2>,
    ) {
        let mask = if enabled {
            0xFF
        } else {
            0x00
        };
        channel_mask.set_bank(0, mask);
        channel_mask.set_bank(1, mask);
        channel_mask.set_bank(2, mask);
        channel_mask.set_bank(3, mask);
        channel_mask.set_bank(4, mask);
        channel_mask.set_bank(5, mask);
        channel_mask.set_bank(6, mask);
        channel_mask.set_bank(7, mask);

        channel_mask.set_bank(8, extra_mask.get_index(0));
        // Bank 9 is not (yet) used for frequencies
        // channel_mask.set_bank(9, extra_mask.get_index(1));
    }

    #[allow(unused)]
    pub fn get_max_payload_length(datarate: DR, repeater_compatible: bool, dwell_time: bool) -> u8 {
        F::get_max_payload_length(datarate, repeater_compatible, dwell_time)
    }
}

pub(crate) trait FixedChannelRegion: ChannelRegion {
    fn uplink_channels() -> &'static [u32; 72];
    fn downlink_channels() -> &'static [u32; 8];
    fn default_rx2_freq() -> u32;
    fn get_rx_datarate(tx_datarate: DR, window: &Window) -> Datarate;
}

impl<F: FixedChannelRegion> RegionHandler for FixedChannelPlan<F> {
    fn process_join_accept<T: AsRef<[u8]>>(&mut self, join_accept: &DecryptedJoinAcceptPayload<T>) {
        if let Some(CfList::FixedChannel(channel_mask)) = join_accept.c_f_list() {
            self.channel_mask_set(channel_mask);
        }
    }

    fn channel_mask_get(&self) -> ChannelMask<9> {
        self.channel_mask.clone()
    }

    fn channel_mask_set(&mut self, channel_mask: ChannelMask<9>) {
        self.join_channels.reset();
        self.channel_mask = channel_mask;
    }

    fn channel_mask_update(
        &self,
        channel_mask: &mut ChannelMask<9>,
        ch_mask_ctl: u8,
        ch_mask: ChannelMask<2>,
    ) -> Option<()> {
        match ch_mask_ctl {
            0..=4 => {
                let base_index = ch_mask_ctl as usize * 2;
                channel_mask.set_bank(base_index, ch_mask.get_index(0));
                channel_mask.set_bank(base_index + 1, ch_mask.get_index(1));
            }
            5 => {
                let ch_mask: u16 =
                    ch_mask.get_index(0) as u16 | ((ch_mask.get_index(1) as u16) << 8);
                channel_mask.set_bank(0, ((ch_mask & 0b1) * 0xFF) as u8);
                channel_mask.set_bank(1, ((ch_mask & 0b10) * 0xFF) as u8);
                channel_mask.set_bank(2, ((ch_mask & 0b100) * 0xFF) as u8);
                channel_mask.set_bank(3, ((ch_mask & 0b1000) * 0xFF) as u8);
                channel_mask.set_bank(4, ((ch_mask & 0b10000) * 0xFF) as u8);
                channel_mask.set_bank(5, ((ch_mask & 0b100000) * 0xFF) as u8);
                channel_mask.set_bank(6, ((ch_mask & 0b1000000) * 0xFF) as u8);
                channel_mask.set_bank(7, ((ch_mask & 0b10000000) * 0xFF) as u8);
                channel_mask.set_bank(8, ((ch_mask & 0b100000000) * 0xFF) as u8);
            }
            6 => {
                self.set_125k_channels(channel_mask, true, ch_mask);
            }
            7 => {
                self.set_125k_channels(channel_mask, false, ch_mask);
            }
            _ => {
                // RFU
                return None;
            }
        }
        Some(())
    }

    fn channel_mask_validate(&self, channel_mask: &ChannelMask<9>, dr: Option<DR>) -> bool {
        if let Some(dr) = dr {
            if let Some(dr) = &F::datarates()[dr as usize] {
                return match dr.bandwidth {
                    Bandwidth::_500KHz => (64..=71).any(|i| channel_mask.is_enabled(i).unwrap()),
                    Bandwidth::_125KHz => {
                        // Check that at least two channels are enabled
                        (0..64).filter(|&i| channel_mask.is_enabled(i).unwrap()).take(2).count()
                            == 2
                    }
                    _ => true,
                };
            }
        }
        false
    }

    fn get_datarate(&self, dr: u8) -> Option<&Datarate> {
        F::datarates()[dr as usize].as_ref()
    }

    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        match frame {
            Frame::Join => {
                let channel = self.join_channels.get_next_channel(rng);
                let dr = if channel < 64 {
                    DR::_0
                } else {
                    DR::_4
                };
                self.last_tx_channel = channel;
                let data_rate = F::datarates()[dr as usize].clone().unwrap();
                (data_rate, F::uplink_channels()[channel as usize])
            }
            Frame::Data => {
                // The join bias gets reset after receiving CFList in Join Frame
                // or ChannelMask in the LinkADRReq in Data Frame.
                // If it has not been reset yet, we continue to use the bias for the data frames.
                // We hope to acquire ChannelMask via LinkADRReq.
                let (data_rate, channel) = if self.join_channels.has_bias_and_not_exhausted() {
                    let channel = self.join_channels.get_next_channel(rng);
                    let dr = if channel < 64 {
                        DR::_0
                    } else {
                        DR::_4
                    };
                    (F::datarates()[dr as usize].clone().unwrap(), channel)
                // Alternatively, we will ask JoinChannel logic to determine a channel from the
                // subband that  the join succeeded on.
                } else if let Some(channel) = self.join_channels.first_data_channel(rng) {
                    (F::datarates()[datarate as usize].clone().unwrap(), channel)
                } else {
                    // For the data frame, the datarate impacts which channel sets we can choose
                    // from. If the datarate bandwidth is 500 kHz, we must use
                    // channels 64..=71. Else, we must use 0-63
                    let datarate = F::datarates()[datarate as usize].clone().unwrap();
                    if datarate.bandwidth == Bandwidth::_500KHz {
                        let mut channel = (rng.next_u32() & 0b111) as u8;
                        // keep selecting a random channel until we find one that is enabled
                        while !self.channel_mask.is_enabled((channel + 64).into()).unwrap() {
                            channel = (rng.next_u32() & 0b111) as u8;
                        }
                        (datarate, 64 + channel)
                    } else {
                        let mut channel = (rng.next_u32() & 0b111111) as u8;
                        // keep selecting a random channel until we find one that is enabled
                        while !self.channel_mask.is_enabled(channel.into()).unwrap() {
                            channel = (rng.next_u32() & 0b111111) as u8;
                        }
                        (datarate, channel)
                    }
                };
                self.last_tx_channel = channel;
                (data_rate, F::uplink_channels()[channel as usize])
            }
        }
    }

    fn get_rx_frequency(&self, _frame: &Frame, window: &Window) -> u32 {
        let channel = self.last_tx_channel % 8;
        match window {
            Window::_1 => F::downlink_channels()[channel as usize],
            Window::_2 => F::default_rx2_freq(),
        }
    }

    fn get_rx_datarate(&self, tx_datarate: DR, window: &Window) -> Datarate {
        F::get_rx_datarate(tx_datarate, window)
    }

    fn check_tx_power(&self, tx_power: u8) -> Option<u8> {
        F::tx_power_adjust(tx_power)
    }

    fn frequency_valid(&self, freq: u32) -> bool {
        (self.frequency_valid)(freq)
    }

    fn has_fixed_channel_plan(&self) -> bool {
        true
    }

    fn channel_dl_update(&mut self, _: u8, _: u32) -> (bool, bool) {
        unreachable!()
    }

    fn handle_new_channel(&mut self, _: u8, _: u32, _: Option<DataRateRange>) -> (bool, bool) {
        unreachable!()
    }
}
