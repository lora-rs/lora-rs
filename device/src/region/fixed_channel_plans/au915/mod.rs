use super::*;

mod frequencies;
use frequencies::*;

mod datarates;
use datarates::*;

const AU_DBM: i8 = 21;
const DEFAULT_RX2: u32 = 923_300_000;

#[derive(Default, Clone)]
pub struct AU915 {
    pub(crate) plan: FixedChannelPlan<16, AU915Region>,
}

impl AU915 {
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
    ///
    /// # About supported channels
    ///
    /// Supported channels:
    ///
    /// * 64 125 kHz channels (0-63)
    /// * 8 500 kHz channels (64-71)
    ///
    /// If a channel out of this range is specified, `Err(())` will be returned.
    ///
    /// # Returns
    ///
    /// * `Ok(Configuration)` if the provided channel set is correct
    /// * The length of `channel_list` must be <= 16, otherwise `Err` will
    ///   be returned.
    /// * If a channel out of the specified channel range is specified,
    ///   `Err` will be returned (ie, >= 72).
    pub fn set_preferred_join_channels<const N: usize>(
        &mut self,
        preferred_channels: &[Channel; N],
    ) {
        self.set_preferred_join_channels_and_noncompliant_retries(preferred_channels, 1)
    }

    /// Specify a set of channels enabled
    /// for joining the network. You can specify up to 16 preferred channels.
    ///
    /// When `join` is called on a [`Configuration`] created using this
    /// region state, the network will be attempted to be joined only on the provided
    /// channel subset. This set of channels will be retried the number of times specified; after which we will revert
    /// to trying to join with all channels enabled using a preset sequence.
    ///
    /// # About supported channels
    ///
    /// Supported channels:
    ///
    /// * 64 125 kHz channels (0-63)
    /// * 8 500 kHz channels (64-71)
    ///
    /// If a channel out of this range is specified, `Err(())` will be returned.
    ///
    /// # ⚠️Warning⚠️
    ///
    /// This method is explicitely not compliant with the LoRaWAN spec.
    ///
    /// It is recommended to set a low number (ie, < 10) of join retries using the
    /// preferred channels. The reason for this is if you *only* try to join
    /// with a channel bias, and the network is configured to use a
    /// strictly different set of channels than the ones you provide, the
    /// network will NEVER be joined.
    ///
    /// # Returns
    ///
    /// * `Ok(Configuration)` if the provided channel set is correct
    /// * The length of `channel_list` must be <= 16, otherwise `Err` will
    ///   be returned.
    /// * If a channel out of the specified channel range is specified,
    ///   `Err` will be returned (ie, >= 72).
    pub fn set_preferred_join_channels_and_noncompliant_retries<const N: usize>(
        &mut self,
        preferred_channels: &[Channel; N],
        num_retries: usize,
    ) {
        gen_assert!(N, N <= 16);

        self.plan.set_preferred_join_channels(preferred_channels, num_retries);
    }
}

impl RegionHandler for AU915 {
    #[inline]
    fn process_join_accept<T: AsRef<[u8]>, C>(
        &mut self,
        join_accept: &DecryptedJoinAcceptPayload<T, C>,
    ) {
        self.plan.process_join_accept(join_accept);
    }

    #[inline]
    fn handle_link_adr_channel_mask(
        &mut self,
        channel_mask_control: u8,
        channel_mask: ChannelMask<2>,
    ) {
        self.plan.handle_link_adr_channel_mask(channel_mask_control, channel_mask);
    }

    #[inline]
    fn get_tx_dr_and_frequency<RNG: RngCore>(
        &mut self,
        rng: &mut RNG,
        datarate: DR,
        frame: &Frame,
    ) -> (Datarate, u32) {
        self.plan.get_tx_dr_and_frequency(rng, datarate, frame)
    }

    #[inline]
    fn get_rx_frequency(&self, frame: &Frame, window: &Window) -> u32 {
        self.plan.get_rx_frequency(frame, window)
    }

    #[inline]
    fn get_rx_datarate(&self, datarate: DR, frame: &Frame, window: &Window) -> Datarate {
        self.plan.get_rx_datarate(datarate, frame, window)
    }
}

#[derive(Default, Clone)]
pub(crate) struct AU915Region;

impl FixedChannelRegion<16> for AU915Region {
    fn datarates() -> &'static [Option<Datarate>; 16] {
        &DATARATES
    }
    fn uplink_channels() -> &'static [u32; 72] {
        &UPLINK_CHANNEL_MAP
    }
    fn downlink_channels() -> &'static [u32; 8] {
        &DOWNLINK_CHANNEL_MAP
    }
    fn get_default_rx2() -> u32 {
        DEFAULT_RX2
    }
    fn get_rx_datarate(tx_datarate: DR, _frame: &Frame, window: &Window) -> Datarate {
        let datarate = match window {
            Window::_1 => {
                // no support for RX1 DR Offset
                match tx_datarate {
                    DR::_0 => DR::_8,
                    DR::_1 => DR::_9,
                    DR::_2 => DR::_10,
                    DR::_3 => DR::_11,
                    DR::_4 => DR::_12,
                    DR::_5 => DR::_13,
                    DR::_6 => DR::_13,
                    DR::_7 => DR::_9,
                    _ => panic!("Invalid TX datarate"),
                }
            }
            Window::_2 => DR::_8,
        };
        DATARATES[datarate as usize].clone().unwrap()
    }
    fn get_dbm() -> i8 {
        AU_DBM
    }
}
