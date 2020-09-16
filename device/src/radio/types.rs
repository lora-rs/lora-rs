#[derive(Debug)]
pub enum Bandwidth {
    _125KHZ,
    _250KHZ,
    _500KHZ,
}

#[derive(Debug)]
pub enum SpreadingFactor {
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
}

#[derive(Debug)]
pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

#[derive(Debug)]
pub struct RfConfig {
    pub frequency: u32,
    pub bandwidth: Bandwidth,
    pub spreading_factor: SpreadingFactor,
    pub coding_rate: CodingRate,
}

impl Default for RfConfig {
    fn default() -> RfConfig {
        RfConfig {
            frequency: 902_300_000,
            bandwidth: Bandwidth::_500KHZ,
            spreading_factor: SpreadingFactor::_10,
            coding_rate: CodingRate::_4_5,
        }
    }
}

#[derive(Debug)]
pub struct TxConfig {
    pub pw: i8,
    pub rf: RfConfig,
}

impl Default for TxConfig {
    fn default() -> TxConfig {
        TxConfig {
            pw: 20,
            rf: RfConfig::default(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
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
