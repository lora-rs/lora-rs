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
mod eu433;
#[cfg(feature = "region-eu868")]
mod eu868;
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
}

impl Channel {
    /// Initialize Channel with frequency and supported minimum and maximum data rates
    fn new(f: u32, dr_min: DR, dr_max: DR) -> Self {
        Self { frequency: f, _datarates: DataRateRange::new_range(dr_min, dr_max) }
    }
}

type ChannelPlan = [Option<Channel>; NUM_CHANNELS_DYNAMIC as usize];

#[derive(Clone)]
pub(crate) struct DynamicChannelPlan<R: DynamicChannelRegion> {
    channels: ChannelPlan,
    channel_mask: ChannelMask<9>,
    last_tx_channel: u8,
    _fixed_channel_region: PhantomData<R>,
    rx1_offset: usize,
    rx2_dr: usize,

    frequency_valid: fn(u32) -> bool,
}

impl<R: DynamicChannelRegion> DynamicChannelPlan<R> {
    fn new(freq_fn: fn(u32) -> bool) -> Self {
        let mut channels = [None; NUM_CHANNELS_DYNAMIC as usize];
        R::init_channels(&mut channels);

        Self {
            channel_mask: Default::default(),
            channels,
            last_tx_channel: Default::default(),
            _fixed_channel_region: Default::default(),
            rx1_offset: Default::default(),
            rx2_dr: Default::default(),
            frequency_valid: freq_fn,
        }
    }

    fn get_channel(&self, index: usize) -> Option<u32> {
        if let Some(channel) = self.channels[index] {
            return Some(channel.frequency);
        }
        None
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

    pub fn check_data_rate(&self, datarate: u8) -> Option<DR> {
        if (datarate as usize) < NUM_DATARATES.into() && R::datarates()[datarate as usize].is_some()
        {
            return Some(DR::try_from(datarate).unwrap());
        }
        None
    }
}

pub(crate) trait DynamicChannelRegion: ChannelRegion {
    fn join_channels() -> u8;
    fn init_channels(channels: &mut ChannelPlan);
    fn get_default_rx2() -> u32;
}

impl<R: DynamicChannelRegion> RegionHandler for DynamicChannelPlan<R> {
    fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) {
        match join_accept.c_f_list() {
            // Type 0
            Some(CfList::DynamicChannel(cf_list)) => {
                // CfList of Type 0 may contain up to 5 frequencies, which define
                // channels J to (J+4). Data rates for these channels is DR0..=DR5
                for (n, freq) in cf_list.iter().enumerate() {
                    let index = R::join_channels() as usize + n;
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
                // all channels on
                for i in 0..8 {
                    self.channel_mask.set_bank(i, 0xFF);
                }
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
                // There are at most 3 join channels in dynamic regions,
                // keep sampling until we get a valid channel.
                let mut index = (rng.next_u32() & 0b11) as u8;
                while index >= R::join_channels() {
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
                        if let Some(freq) = self.get_channel(channel) {
                            self.last_tx_channel = channel as u8;
                            return (R::datarates()[datarate as usize].clone().unwrap(), freq);
                        }
                    }
                    channel = self.get_random_in_range(rng)
                }
            }
        }
    }

    fn get_rx_frequency(&self, _frame: &Frame, window: &Window) -> u32 {
        match window {
            // TODO: implement RxOffset but first need to implement RxOffset MacCommand
            Window::_1 => self.get_channel(self.last_tx_channel as usize).unwrap(),
            Window::_2 => R::get_default_rx2(),
        }
    }

    fn get_rx_datarate(&self, tx_datarate: DR, _frame: &Frame, window: &Window) -> Datarate {
        let datarate = match window {
            Window::_1 => tx_datarate as usize + self.rx1_offset,
            Window::_2 => self.rx2_dr,
        };
        R::datarates()[datarate].clone().unwrap()
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

    fn handle_new_channel(
        &mut self,
        index: u8,
        freq: u32,
        dr: Option<DataRateRange>,
    ) -> (bool, bool) {
        // Join channels are readonly - these cannot be modified!
        if index < R::join_channels() {
            return (false, false);
        }
        // Disable channel if frequency is 0
        if freq == 0 {
            self.channels[index as usize] = None;
            return (true, true);
        }
        let freq_valid = self.frequency_valid(freq);

        // Check if DataRateRange is valid and supported
        if let Some(r) = dr {
            let dr_supported = (r.min_data_rate()..=r.max_data_rate())
                .all(|c| (R::datarates()[c as usize]).is_some());

            if freq_valid && dr_supported {
                self.channels[index as usize] = Some(Channel { frequency: freq, _datarates: r });
            }
            return (freq_valid, dr_supported);
        }
        (freq_valid, false)
    }
}
