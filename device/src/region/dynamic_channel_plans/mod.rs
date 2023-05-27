use super::*;
use crate::RngBufferEmpty;
use core::marker::PhantomData;

mod as923;
mod eu433;
mod eu868;
mod in865;

pub(crate) use as923::AS923_1;
pub(crate) use as923::AS923_2;
pub(crate) use as923::AS923_3;
pub(crate) use as923::AS923_4;
pub(crate) use eu433::EU433;
pub(crate) use eu868::EU868;
pub(crate) use in865::IN865;

#[derive(Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct DynamicChannelPlan<
    const NUM_JOIN_CHANNELS: usize,
    const NUM_DATARATES: usize,
    R: DynamicChannelRegion<NUM_JOIN_CHANNELS, NUM_DATARATES>,
> {
    additional_channels: [Option<u32>; 5],
    channel_mask: ChannelMask<9>,
    last_tx_channel: u8,
    _fixed_channel_region: PhantomData<R>,
    rx1_offset: usize,
    rx2_dr: usize,
}

impl<
        const NUM_JOIN_CHANNELS: usize,
        const NUM_DATARATES: usize,
        R: DynamicChannelRegion<NUM_JOIN_CHANNELS, NUM_DATARATES>,
    > DynamicChannelPlan<NUM_JOIN_CHANNELS, NUM_DATARATES, R>
{
    fn get_channel(&self, channel: usize) -> Option<u32> {
        if channel < NUM_JOIN_CHANNELS {
            Some(R::join_channels()[channel])
        } else {
            self.additional_channels[channel - NUM_JOIN_CHANNELS]
        }
    }

    /// Number of enabled additional channels
    fn num_additional_channels(&self) -> usize {
        self.additional_channels
            .iter()
            .filter(|c| c.is_some())
            .count()
    }

    fn get_random_in_range<RNG: GetRandom>(&self, rng: &mut RNG) -> Result<usize, RngBufferEmpty> {
        let range = NUM_JOIN_CHANNELS + self.num_additional_channels();
        Ok(rng.get_random()? % range)
    }
}

pub(crate) trait DynamicChannelRegion<const NUM_JOIN_CHANNELS: usize, const NUM_DATARATES: usize> {
    fn join_channels() -> [u32; NUM_JOIN_CHANNELS];
    fn datarates() -> &'static [Option<Datarate>; NUM_DATARATES];
    fn get_default_rx2() -> u32;
}

impl<
        const NUM_JOIN_CHANNELS: usize,
        const NUM_DATARATES: usize,
        R: DynamicChannelRegion<NUM_JOIN_CHANNELS, NUM_DATARATES>,
    > RegionHandler for DynamicChannelPlan<NUM_JOIN_CHANNELS, NUM_DATARATES, R>
{
    fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) {
        match join_accept.c_f_list() {
            Some(CfList::DynamicChannel(cf_list)) => {
                // If CfList of Type 0 is present, it may contain up to 5 frequencies
                // which define channels J to (J+4)
                for (index, freq) in cf_list.iter().enumerate() {
                    let value = freq.value();
                    // unused channels are set to 0
                    if value != 0 {
                        self.additional_channels[index] = Some(value);
                    } else {
                        self.additional_channels[index] = None;
                    }
                }
            }
            Some(CfList::FixedChannel(_cf_list)) => {
                //TODO: dynamic channel plans have corresponding fixed channel lists,
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
            0 | 1 | 2 | 3 | 4 => {
                let base_index = channel_mask_control as usize * 2;
                self.channel_mask
                    .set_bank(base_index, channel_mask.get_index(0));
                self.channel_mask
                    .set_bank(base_index + 1, channel_mask.get_index(1));
            }
            5 => {
                let channel_mask: u16 =
                    channel_mask.get_index(0) as u16 | ((channel_mask.get_index(1) as u16) << 8);
                self.channel_mask
                    .set_bank(0, ((channel_mask & 0b1) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(1, ((channel_mask & 0b10) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(2, ((channel_mask & 0b100) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(3, ((channel_mask & 0b1000) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(4, ((channel_mask & 0b10000) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(5, ((channel_mask & 0b100000) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(6, ((channel_mask & 0b1000000) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(7, ((channel_mask & 0b10000000) * 0xFF) as u8);
                self.channel_mask
                    .set_bank(8, ((channel_mask & 0b100000000) * 0xFF) as u8);
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

    /// Needs a RNG buffer holding at least 1 random number.
    fn get_tx_dr_and_frequency<RNG: GetRandom>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> Result<(Datarate, u32), RngBufferEmpty> {
        let random_number = rng.get_random()?;
        let datarate = R::datarates()[datarate as usize].clone().unwrap();

        match frame {
            Frame::Join => {
                // Select a join channel depending on the frequency plan. There are at most 8 join channels.
                let channel = random_number % core::cmp::min(NUM_JOIN_CHANNELS, 8);

                self.last_tx_channel = channel as u8;
                Ok((datarate, R::join_channels()[channel]))
            }

            Frame::Data => {
                let enabled_channels = self.channel_mask.as_vec();
                let channel_index = self.get_random_in_range(rng)?;
                let channel = enabled_channels[channel_index];
                // Unwrapping here should be OK: if self.channel_mask and self.additional_channels are in sync, we
                // should only ever try to get channels that are enabled, therefore are Some in self.
                // additional_channels.
                let freq = self.get_channel(channel as usize).unwrap();

                self.last_tx_channel = channel;
                Ok((datarate, freq))
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
}
