use heapless::consts::*;
use heapless::Vec;
use sx12xx;

pub enum Bandwidth {
    _125KHZ,
    _250KHZ,
    _500KHZ,
}

pub enum SpreadingFactor {
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
}

pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

#[derive(Copy, Clone, Debug)]
pub struct RxQuality {
    rssi: i16,
    snr: i8,
}

pub enum State {
    Busy,
    TxDone,
    RxDone(RxQuality),
    TxError,
    RxError,
}

pub trait Radio {
    type Event;
    fn send(&mut self, buffer: &[u8]);
    fn send_buffer(&mut self);
    fn set_frequency(&mut self, frequency_mhz: u32);
    fn get_mut_buffer(&mut self) -> &mut Vec<u8, U256>;
    fn get_received_packet(&mut self) -> &mut Vec<u8, U256>;
    fn clear_buffer(&mut self);
    fn configure_tx(
        &mut self,
        power: i8,
        bandwidth: Bandwidth,
        datarate: SpreadingFactor,
        coderate: CodingRate,
    );
    fn configure_rx(
        &mut self,
        bandwidth: Bandwidth,
        spreading_factor: SpreadingFactor,
        coderate: CodingRate,
    );
    fn set_rx(&mut self);
    fn handle_event(&mut self, event: Self::Event) -> State;
}

impl Radio for sx12xx::Sx12xx {
    type Event = sx12xx::Event;

    fn send(&mut self, buffer: &[u8]) {
        self.send(buffer)
    }

    fn clear_buffer(&mut self) {
        self.clear_buffer()
    }


    fn send_buffer(&mut self) {
        self.send_buffer()
    }

    fn set_frequency(&mut self, frequency_mhz: u32) {
        self.set_frequency(frequency_mhz)
    }

    fn get_mut_buffer(&mut self) -> &mut Vec<u8, U256> {
        self.get_mut_buffer()
    }

    fn get_received_packet(&mut self) -> &mut Vec<u8, U256> {
        self.get_mut_buffer()
    }

    fn configure_tx(
        &mut self,
        power: i8,
        bandwidth: Bandwidth,
        spreading_factor: SpreadingFactor,
        coderate: CodingRate,
    ) {
        self.configure_lora_tx(
            power,
            bandwidth.into(),
            spreading_factor.into(),
            coderate.into(),
        );
    }

    fn configure_rx(
        &mut self,
        bandwidth: Bandwidth,
        spreading_factor: SpreadingFactor,
        coderate: CodingRate,
    ) {
        self.configure_lora_rx(bandwidth.into(), spreading_factor.into(), coderate.into());
    }

    fn set_rx(&mut self) {
        self.set_rx();
    }

    fn handle_event(&mut self, event: Self::Event) -> State {
        self.handle_event(event).into()
    }
}

impl Into<sx12xx::LoRaBandwidth> for Bandwidth {
    fn into(self: Bandwidth) -> sx12xx::LoRaBandwidth {
        match self {
            Bandwidth::_125KHZ => sx12xx::LoRaBandwidth::_125KHZ,
            Bandwidth::_250KHZ => sx12xx::LoRaBandwidth::_250KHZ,
            Bandwidth::_500KHZ => sx12xx::LoRaBandwidth::_500KHZ,
        }
    }
}

impl Into<sx12xx::LoRaSpreadingFactor> for SpreadingFactor {
    fn into(self: SpreadingFactor) -> sx12xx::LoRaSpreadingFactor {
        match self {
            SpreadingFactor::_7 => sx12xx::LoRaSpreadingFactor::_7,
            SpreadingFactor::_8 => sx12xx::LoRaSpreadingFactor::_8,
            SpreadingFactor::_9 => sx12xx::LoRaSpreadingFactor::_9,
            SpreadingFactor::_10 => sx12xx::LoRaSpreadingFactor::_10,
            SpreadingFactor::_11 => sx12xx::LoRaSpreadingFactor::_11,
            SpreadingFactor::_12 => sx12xx::LoRaSpreadingFactor::_12,
        }
    }
}

impl Into<sx12xx::LoRaCodingRate> for CodingRate {
    fn into(self: CodingRate) -> sx12xx::LoRaCodingRate {
        match self {
            CodingRate::_4_5 => sx12xx::LoRaCodingRate::_4_5,
            CodingRate::_4_6 => sx12xx::LoRaCodingRate::_4_6,
            CodingRate::_4_7 => sx12xx::LoRaCodingRate::_4_7,
            CodingRate::_4_8 => sx12xx::LoRaCodingRate::_4_8,
        }
    }
}

// impl Into<sx12xx::State> for State {
//     fn from(self: State) -> sx12xx::State {
//         match self {
//             State::Busy =>  sx12xx::State::Sx12xxState_Busy,
//             State::TxDone => sx12xx::State::Sx12xxState_TxDone,
//             State::RxDone => sx12xx::State::Sx12xxState_RxDone,
//         }
//     }
// }

impl From<sx12xx::State> for State {
    fn from(state: sx12xx::State) -> State {
        match state {
            sx12xx::State::Busy => State::Busy,
            sx12xx::State::TxDone => State::TxDone,
            sx12xx::State::RxDone(quality) => State::RxDone(RxQuality {
                snr: quality.get_snr(),
                rssi: quality.get_rssi(),
            }),
            sx12xx::State::TxTimeout => State::TxError,
            sx12xx::State::RxTimeout => State::RxError,
            sx12xx::State::RxError => State::RxError,
        }
    }
}
