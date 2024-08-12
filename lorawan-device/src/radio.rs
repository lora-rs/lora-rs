pub use lora_modulation::BaseBandModulationParams;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RfConfig {
    pub frequency: u32,
    pub bb: BaseBandModulationParams,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RxMode {
    Continuous,
    /// Single shot receive. Argument `ms` indicates how many milliseconds of extra buffer time should
    /// be added to the preamble detection timeout.
    Single {
        ms: u32,
    },
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RxConfig {
    pub rf: RfConfig,
    pub mode: RxMode,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TxConfig {
    pub pw: i8,
    pub rf: RfConfig,
}

impl TxConfig {
    pub fn adjust_power(&mut self, max_power: u8, antenna_gain: i8) {
        self.pw -= antenna_gain;
        self.pw = core::cmp::min(self.pw, max_power as i8);
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RxQuality {
    rssi: i16,
    snr: i8,
}

impl RxQuality {
    pub fn new(rssi: i16, snr: i8) -> RxQuality {
        RxQuality { rssi, snr }
    }

    pub fn rssi(self) -> i16 {
        self.rssi
    }
    pub fn snr(self) -> i8 {
        self.snr
    }
}

pub(crate) struct RadioBuffer<const N: usize> {
    packet: [u8; N],
    pos: usize,
}

impl<const N: usize> RadioBuffer<N> {
    pub(crate) fn new() -> Self {
        Self { packet: [0; N], pos: 0 }
    }

    pub(crate) fn clear(&mut self) {
        self.pos = 0;
    }

    pub(crate) fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub(crate) fn extend_from_slice(&mut self, buf: &[u8]) -> Result<(), ()> {
        if self.pos + buf.len() < self.packet.len() {
            self.packet[self.pos..self.pos + buf.len()].copy_from_slice(buf);
            self.pos += buf.len();
            Ok(())
        } else {
            Err(())
        }
    }

    /// Provides a mutable slice of the buffer up to the current position.
    pub(crate) fn as_mut_for_read(&mut self) -> &mut [u8] {
        &mut self.packet[..self.pos]
    }

    /// Provides a reference of the buffer up to the current position.

    pub(crate) fn as_ref_for_read(&self) -> &[u8] {
        &self.packet[..self.pos]
    }
}

impl<const N: usize> AsMut<[u8]> for RadioBuffer<N> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.packet
    }
}

impl<const N: usize> AsRef<[u8]> for RadioBuffer<N> {
    fn as_ref(&self) -> &[u8] {
        &self.packet
    }
}
