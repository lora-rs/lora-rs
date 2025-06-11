use super::*;
use core::marker::PhantomData;
use lorawan::types::DataRateRange;

#[cfg(any(
    feature = "region-as923-1",
    feature = "region-as923-2",
    feature = "region-as923-3",
    feature = "region-as923-4"
))]
mod as923;
#[cfg(feature = "region-eu433")]
pub(crate) mod eu433;
#[cfg(feature = "region-eu868")]
pub(crate) mod eu868;
#[cfg(feature = "region-in865")]
mod in865;

#[cfg(feature = "region-as923-1")]
pub(crate) use as923::AS923_1;
#[cfg(feature = "region-as923-2")]
pub(crate) use as923::AS923_2;
#[cfg(feature = "region-as923-3")]
pub(crate) use as923::AS923_3;
#[cfg(feature = "region-as923-4")]
pub(crate) use as923::AS923_4;
#[cfg(feature = "region-eu433")]
pub(crate) use eu433::EU433;
#[cfg(feature = "region-eu868")]
pub(crate) use eu868::EU868;
#[cfg(feature = "region-in865")]
pub(crate) use in865::IN865;

#[derive(Clone, Copy)]
pub(crate) struct Channel {
    frequency: u32,
    _datarates: DataRateRange,
    dl_frequency: Option<u32>,
}

impl Channel {
    /// Initialize Channel with frequency and supported minimum and maximum data rates
    fn new(f: u32, dr_min: DR, dr_max: DR) -> Self {
        Self::new_with_dr(f, DataRateRange::new_range(dr_min, dr_max))
    }

    fn new_with_dr(f: u32, dr: DataRateRange) -> Self {
        Self { frequency: f, _datarates: dr, dl_frequency: None }
    }

    fn rx1_frequency(&self) -> u32 {
        self.dl_frequency.unwrap_or(self.frequency)
    }

    fn ul_frequency(&self) -> u32 {
        self.frequency
    }
}

type ChannelPlan = [Option<Channel>; NUM_CHANNELS_DYNAMIC as usize];

#[derive(Clone)]
pub(crate) struct DynamicChannelPlan<R: DynamicChannelRegion> {
    channels: ChannelPlan,
    channel_mask: ChannelMask<9>,
    last_tx_channel: u8,
    _dynamic_channel_region: PhantomData<R>,
    frequency_valid: fn(u32) -> bool,
}

impl<R: DynamicChannelRegion> DynamicChannelPlan<R> {
    fn new(freq_fn: fn(u32) -> bool) -> Self {
        let mut channels = [None; NUM_CHANNELS_DYNAMIC as usize];
        R::init_channels(&mut channels);

        Self {
            channels,
            channel_mask: Default::default(),
            last_tx_channel: Default::default(),
            _dynamic_channel_region: Default::default(),
            frequency_valid: freq_fn,
        }
    }

    fn get_random_in_range<RNG: RngCore>(&self, rng: &mut RNG) -> usize {
        // SAFETY: We will always have at least number of join channels, therefore
        // unwrap is safe to use.
        let range = self.channels.iter().rposition(|&x| x.is_some()).unwrap() + 1;
        let cm = if range > 16 {
            0b11111
        } else if range > 8 {
            0b1111
        } else {
            0b111
        };
        (rng.next_u32() as usize) & cm
    }

    pub fn get_max_payload_length(datarate: DR, repeater_compatible: bool, dwell_time: bool) -> u8 {
        R::get_max_payload_length(datarate, repeater_compatible, dwell_time)
    }
}

pub(crate) trait DynamicChannelRegion: ChannelRegion {
    const NUM_JOIN_CHANNELS: u8;
    fn init_channels(channels: &mut ChannelPlan);
    fn get_rx_datarate(tx_datarate: DR, rx1_dr_offset: u8, window: &Window) -> DR;
}

impl<R: DynamicChannelRegion> RegionHandler for DynamicChannelPlan<R> {
    fn process_join_accept<T: AsRef<[u8]>>(&mut self, join_accept: &DecryptedJoinAcceptPayload<T>) {
        match join_accept.c_f_list() {
            // Type 0
            Some(CfList::DynamicChannel(cf_list)) => {
                // CfList of Type 0 may contain up to 5 frequencies, which define
                // channels J to (J+4). Data rates for these channels is DR0..=DR5
                for (n, freq) in cf_list.iter().enumerate() {
                    let index = R::NUM_JOIN_CHANNELS as usize + n;
                    let value = freq.value();
                    // unused channels are set to 0
                    if value == 0 {
                        self.channels[index] = None;
                    } else {
                        self.channels[index] = Some(Channel::new(value, DR::_0, DR::_5));
                    }
                }
            }
            // Type 1
            Some(CfList::FixedChannel(_cf_list)) => {
                // TODO: dynamic channel plans have corresponding fixed channel lists,
                // however, this feature is entirely optional
            }
            None => {}
        }
    }

    fn channel_mask_get(&self) -> ChannelMask<9> {
        self.channel_mask.clone()
    }

    fn channel_mask_set(&mut self, channel_mask: ChannelMask<9>) {
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
                // all channels on
                for i in 0..8 {
                    channel_mask.set_bank(i, 0xFF);
                }
            }
            _ => {
                // RFU
                return None;
            }
        }
        Some(())
    }

    fn channel_mask_validate(&self, channel_mask: &ChannelMask<9>, _dr: Option<DR>) -> bool {
        // TODO: We should also check whether DR and txpower for all(?) channels is valid
        (0..NUM_CHANNELS_DYNAMIC).any(|i| {
            if channel_mask.is_enabled(i as usize).unwrap() {
                self.channels[i as usize].is_some()
            } else {
                false
            }
        })
        // (2..9).all(|i| channel_mask.get_index(i) == 0)
    }

    fn get_datarate(&self, dr: u8) -> Option<&Datarate> {
        R::datarates()[dr as usize].as_ref()
    }

    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        match frame {
            Frame::Join => {
                // There are at most 3 join channels in dynamic regions,
                // keep sampling until we get a valid channel.
                let mut index = (rng.next_u32() & 0b11) as u8;
                while index >= R::NUM_JOIN_CHANNELS {
                    index = (rng.next_u32() & 0b11) as u8;
                }
                self.last_tx_channel = index;

                // SAFETY: Join channels SHALL be always present
                let channel = self.channels[index as usize].unwrap();
                (R::datarates()[datarate as usize].clone().unwrap(), channel.frequency)
            }
            Frame::Data => {
                let mut channel = self.get_random_in_range(rng);
                loop {
                    if self.channel_mask.is_enabled(channel).unwrap() {
                        if let Some(ch) = self.channels[channel] {
                            self.last_tx_channel = channel as u8;
                            return (
                                R::datarates()[datarate as usize].clone().unwrap(),
                                ch.ul_frequency(),
                            );
                        }
                    }
                    channel = self.get_random_in_range(rng)
                }
            }
        }
    }

    fn get_rx_frequency(&self, _frame: &Frame, window: &Window) -> u32 {
        match window {
            // SAFETY: self.last_tx_channel will be populated after correct channel is chosen
            Window::_1 => self.channels[self.last_tx_channel as usize].unwrap().rx1_frequency(),
            Window::_2 => R::DEFAULT_RX2_FREQ,
        }
    }

    fn get_rx_datarate(&self, tx_datarate: DR, rx1_dr_offset: u8, window: &Window) -> DR {
        R::get_rx_datarate(tx_datarate, rx1_dr_offset, window)
    }

    fn check_tx_power(&self, tx_power: u8) -> Option<u8> {
        R::tx_power_adjust(tx_power)
    }

    fn frequency_valid(&self, freq: u32) -> bool {
        (self.frequency_valid)(freq)
    }

    fn has_fixed_channel_plan(&self) -> bool {
        false
    }

    /// Update channel's downlink frequency for RX1 slot
    fn channel_dl_update(&mut self, index: u8, freq: u32) -> (bool, bool) {
        let freq_valid = self.frequency_valid(freq);
        if self.channel_mask.is_enabled(index as usize).is_ok()
            && self.channel_mask.is_enabled(index as usize).unwrap()
        {
            if let Some(mut channel) = self.channels[index as usize] {
                if channel.frequency != 0 {
                    channel.dl_frequency = if freq == channel.frequency {
                        // Reset downlink frequency
                        None
                    } else {
                        // Update downlink frequency
                        Some(freq)
                    };
                    self.channels[index as usize] = Some(channel);
                    return (freq_valid, true);
                }
            }
        }
        (freq_valid, false)
    }

    fn handle_new_channel(
        &mut self,
        index: u8,
        freq: u32,
        dr: Option<DataRateRange>,
    ) -> (bool, bool) {
        // Join channels are readonly - these cannot be modified!
        if index < R::NUM_JOIN_CHANNELS {
            return (false, false);
        }
        // Disable channel if frequency is 0
        if freq == 0 {
            self.channels[index as usize] = None;
            self.channel_mask.set_channel(index as usize, false);
            return (true, true);
        }
        let freq_valid = self.frequency_valid(freq);

        // Check if DataRateRange is valid and supported
        if let Some(r) = dr {
            let dr_supported = (r.min_data_rate()..=r.max_data_rate())
                .all(|c| (R::datarates()[c as usize]).is_some());

            if freq_valid && dr_supported {
                self.channels[index as usize] = Some(Channel::new_with_dr(freq, r));
                self.channel_mask.set_channel(index as usize, true);
            }
            return (freq_valid, dr_supported);
        }
        (freq_valid, false)
    }

    fn rx1_dr_offset_validate(&self, value: u8) -> Option<u8> {
        if value <= R::MAX_RX1_DR_OFFSET {
            Some(value)
        } else {
            None
        }
    }
}
