//! LoRaWAN type primitives (frequency, channelmask, etc)
//! commonly used in payloads.
use crate::maccommands::Error;

/// ChannelMask represents the ChannelMask from LoRaWAN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMask<const N: usize>([u8; N]);

impl<const N: usize> Default for ChannelMask<N> {
    fn default() -> Self {
        ChannelMask([0xFF; N])
    }
}

#[cfg(feature = "serde")]
impl<const N: usize> serde::Serialize for ChannelMask<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for e in &self.0 {
            seq.serialize_element(e)?;
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
struct ChannelMaskDeserializer<const N: usize>;

#[cfg(feature = "serde")]
impl<'de, const N: usize> serde::de::Visitor<'de> for ChannelMaskDeserializer<N> {
    type Value = ChannelMask<N>;

    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ChannelMask byte.")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut arr = [0; N];
        let mut index = 0;
        while let Some(el) = seq.next_element()? {
            if index >= N {
                return Err(serde::de::Error::custom("ChannelMask has too many elements"));
            } else {
                arr[index] = el;
                index += 1;
            }
        }
        Ok(ChannelMask(arr))
    }
}

#[cfg(feature = "serde")]
impl<'de, const N: usize> serde::Deserialize<'de> for ChannelMask<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(ChannelMaskDeserializer {})
    }
}

impl<const N: usize> ChannelMask<N> {
    /// Constructs a new ChannelMask from the provided data.
    pub fn new(data: &[u8]) -> Result<Self, Error> {
        if data.len() < N {
            return Err(Error::BufferTooShort);
        }
        Ok(Self::new_from_raw(data))
    }

    pub fn set_bank(&mut self, index: usize, value: u8) {
        self.0[index] = value;
    }

    /// Enable or disable a specific channel. Recall that LoRaWAN channel numbers start indexing
    /// at zero.
    ///
    /// Improper use of this method could lead to out of bounds panic during runtime!
    pub fn set_channel(&mut self, channel: usize, set: bool) {
        let index = channel >> 3;
        let mut flag = 0b1 << (channel & 0x07);
        if set {
            self.0[index] |= flag;
        } else {
            flag = !flag;
            self.0[index] &= flag;
        }
    }

    pub fn get_index(&self, index: usize) -> u8 {
        self.0[index]
    }

    /// Constructs a new ChannelMask from the provided data, without verifying if they are
    /// admissible.
    ///
    /// Improper use of this method could lead to panic during runtime!
    pub fn new_from_raw(data: &[u8]) -> Self {
        let mut payload = [0; N];
        payload[..N].copy_from_slice(&data[..N]);
        ChannelMask(payload)
    }

    fn channel_enabled(&self, index: usize) -> bool {
        self.0[index >> 3] & (1 << (index & 0x07)) != 0
    }

    /// Verifies if a given channel is enabled.
    pub fn is_enabled(&self, index: usize) -> Result<bool, Error> {
        let index_limit = N * 8 - 1;
        if index > index_limit {
            return Err(Error::InvalidIndex);
        }
        Ok(self.channel_enabled(index))
    }

    /// Provides information for each of the 16 channels if they are enabled.
    pub fn statuses<const M: usize>(&self) -> [bool; M] {
        let mut res = [false; M];
        for (i, c) in res.iter_mut().enumerate() {
            *c = self.channel_enabled(i);
        }
        res
    }
}

impl<const N: usize> From<[u8; N]> for ChannelMask<N> {
    fn from(v: [u8; N]) -> Self {
        ChannelMask(v)
    }
}

impl<const N: usize> AsRef<[u8]> for ChannelMask<N> {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

/// DataRateRange represents LoRaWAN DataRateRange.
#[derive(Debug, PartialEq, Eq)]
pub struct DataRateRange(u8);

impl DataRateRange {
    /// Constructs a new DataRateRange from the provided byte, without checking for correctness.
    pub fn new_from_raw(byte: u8) -> DataRateRange {
        DataRateRange(byte)
    }

    /// Constructs a new DataRateRange from the provided byte.
    pub fn new(byte: u8) -> Result<DataRateRange, Error> {
        Self::can_build_from(byte)?;

        Ok(Self::new_from_raw(byte))
    }

    /// Check if the byte can be used to create DataRateRange.
    pub fn can_build_from(byte: u8) -> Result<(), Error> {
        if (byte >> 4) < (byte & 0x0f) {
            return Err(Error::InvalidDataRateRange);
        }
        Ok(())
    }

    /// The highest data rate allowed on this channel.
    pub fn max_data_rate(&self) -> u8 {
        self.0 >> 4
    }

    /// The lowest data rate allowed on this channel.
    pub fn min_data_rate(&self) -> u8 {
        self.0 & 0x0f
    }

    /// The integer value of the DataRateRange.
    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for DataRateRange {
    fn from(v: u8) -> Self {
        DataRateRange(v)
    }
}

/// DLSettings represents LoRaWAN DLSettings.
#[derive(Debug, PartialEq, Eq)]
pub struct DLSettings(u8);

impl DLSettings {
    /// Constructs a new DLSettings from the provided data.
    pub fn new(byte: u8) -> DLSettings {
        DLSettings(byte)
    }

    /// The offset between the uplink data rate and the downlink data rate used to communicate with
    /// the end-device on the first reception slot (RX1).
    pub fn rx1_dr_offset(&self) -> u8 {
        (self.0 >> 4) & 0x07
    }

    /// The data rate of a downlink using the second receive window.
    pub fn rx2_data_rate(&self) -> u8 {
        self.0 & 0x0f
    }

    /// The integer value of the DL Settings.
    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for DLSettings {
    fn from(v: u8) -> Self {
        DLSettings(v)
    }
}

/// Frequency represents a channel's central frequency.
#[derive(Debug, PartialEq, Eq)]
pub struct Frequency<'a>(&'a [u8]);

impl<'a> Frequency<'a> {
    /// Constructs a new Frequency from the provided bytes, without verifying if they are
    /// admissible.
    ///
    /// Improper use of this method could lead to panic during runtime!
    pub fn new_from_raw(bytes: &'a [u8]) -> Self {
        Frequency(bytes)
    }

    /// Constructs a new Frequency from the provided bytes.
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() != 3 {
            return None;
        }

        Some(Frequency(bytes))
    }

    /// Provides the decimal value in Hz of the frequency.
    pub fn value(&self) -> u32 {
        ((u32::from(self.0[2]) << 16) + (u32::from(self.0[1]) << 8) + u32::from(self.0[0])) * 100
    }
}

impl<'a> From<&'a [u8; 3]> for Frequency<'a> {
    fn from(v: &'a [u8; 3]) -> Self {
        Frequency(&v[..])
    }
}

impl AsRef<[u8]> for Frequency<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

/// Redundancy represents the LinkADRReq Redundancy from LoRaWAN.
#[derive(Debug, PartialEq, Eq)]
pub struct Redundancy(u8);

impl Redundancy {
    /// Constructs a new Redundancy from the provided data.
    pub fn new(data: u8) -> Self {
        Redundancy(data)
    }

    /// Controls the interpretation of the previously defined ChannelMask bit mask.
    pub fn channel_mask_control(&self) -> u8 {
        (self.0 >> 4) & 0x07
    }

    /// How many times each message should be repeated.
    pub fn number_of_transmissions(&self) -> u8 {
        self.0 & 0x0f
    }

    /// The integer value of the Redundancy.
    pub fn raw_value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for Redundancy {
    fn from(v: u8) -> Self {
        Redundancy(v)
    }
}
