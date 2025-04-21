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

    #[allow(unused)]
    pub fn get_max_payload_length(datarate: DR, repeater_compatible: bool, dwell_time: bool) -> u8 {
        F::get_max_payload_length(datarate, repeater_compatible, dwell_time)
    }

    pub fn check_data_rate(&self, datarate: u8) -> Option<DR> {
        if (datarate as usize) < NUM_DATARATES.into() && F::datarates()[datarate as usize].is_some()
        {
            return Some(DR::try_from(datarate).unwrap());
        }
        None
    }
}

pub(crate) trait FixedChannelRegion: ChannelRegion {
    fn uplink_channels() -> &'static [u32; 72];
    fn downlink_channels() -> &'static [u32; 8];
    fn get_default_rx2() -> u32;
    fn get_rx_datarate(tx_datarate: DR, frame: &Frame, window: &Window) -> Datarate;
}

impl<F: FixedChannelRegion> RegionHandler for FixedChannelPlan<F> {
    fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) {
        if let Some(CfList::FixedChannel(channel_mask)) = join_accept.c_f_list() {
            // Reset the join channels state
            self.join_channels.reset();
            self.channel_mask = channel_mask;
        }
    }

    fn handle_link_adr_channel_mask(
        &mut self,
        channel_mask_control: u8,
        channel_mask: ChannelMask<2>,
    ) {
        self.join_channels.reset();
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
                    // channels 64-71. Else, we must use 0-63
                    let datarate = F::datarates()[datarate as usize].clone().unwrap();
                    if datarate.bandwidth == Bandwidth::_500KHz {
                        let mut channel = (rng.next_u32() & 0b111) as u8;
                        // keep selecting a random channel until we find one that is enabled
                        while !self.channel_mask.is_enabled(channel.into()).unwrap() {
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
            Window::_2 => F::get_default_rx2(),
        }
    }

    fn get_rx_datarate(&self, tx_datarate: DR, frame: &Frame, window: &Window) -> Datarate {
        F::get_rx_datarate(tx_datarate, frame, window)
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

    fn handle_new_channel(&mut self, _: u8, _: u32, _: Option<DataRateRange>) -> (bool, bool) {
        unreachable!()
    }
}
