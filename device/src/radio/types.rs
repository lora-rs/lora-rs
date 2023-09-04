use super::*;
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RfConfig {
    pub frequency: u32,
    pub bandwidth: Bandwidth,
    pub spreading_factor: SpreadingFactor,
    pub coding_rate: CodingRate,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TxConfig {
    pub pw: i8,
    pub rf: RfConfig,
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

    pub(crate) fn extend_from_slice(&mut self, buf: &[u8]) -> Result<(), ()> {
        if self.pos + buf.len() < self.packet.len() {
            self.packet[self.pos..self.pos + buf.len()].copy_from_slice(buf);
            self.pos += buf.len();
            Ok(())
        } else {
            Err(())
        }
    }

    #[cfg(feature = "async")]
    pub(crate) fn as_raw_slice(&mut self) -> &mut [u8] {
        &mut self.packet
    }

    #[cfg(feature = "async")]
    pub(crate) fn inc(&mut self, len: usize) {
        assert!(self.pos + len < self.packet.len());
        self.pos += len;
    }
}

impl<const N: usize> AsMut<[u8]> for RadioBuffer<N> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.packet[..self.pos]
    }
}

impl<const N: usize> AsRef<[u8]> for RadioBuffer<N> {
    fn as_ref(&self) -> &[u8] {
        &self.packet[..self.pos]
    }
}
