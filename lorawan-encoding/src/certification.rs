//! LoRaWAN Certification Protocol (TS009) command and payload handling
use lorawan_macros::CommandHandler;

use crate::creator::UnimplementedCreator;
use crate::maccommands::MacCommandIterator;
use crate::maccommands::SerializableMacCommand;

use crate::maccommands::Error;

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum DownlinkDUTCommand<'a> {
    /// Request to reset the Microcontroller Unit
    #[cmd(cid = 0x01, len = 0)]
    DutResetReq(DutResetReqPayload),

    /// Request to reset the LoRaWAN MAC layer and start issuing Join-Request messages
    #[cmd(cid = 0x02, len = 0)]
    DutJoinReq(DutJoinReqPayload),

    /// Request to activate/deactivate Adaptive Data Rate (ADR)
    #[cmd(cid = 0x04, len = 1)]
    AdrBitChangeReq(AdrBitChangeReqPayload<'a>),

    /// Change uplink periodicity to the provided value
    #[cmd(cid = 0x06, len = 1)]
    TxPeriodicityChangeReq(TxPeriodicityChangeReqPayload<'a>),

    /// Send all subsequent uplinks of the specified type
    // NB! Variable length payload without any size indication
    #[cmd(cid = 0x07)]
    TxFramesCtrlReq(TxFramesCtrlReqPayload<'a>),

    /// Requests the DUT to echo the provided payload, where each byte is incremented by 1
    #[cmd(cid = 0x08)]
    EchoIncPayloadReq(EchoIncPayloadReqPayload<'a>),

    /// Requests the DUT to provide the current RxAppCnt value.
    #[cmd(cid = 0x09, len = 0)]
    RxAppCntReq(RxAppCntReqPayload),

    /// Requests the DUT to send a LinkCheckReq MAC command.
    #[cmd(cid = 0x20, len = 0)]
    LinkCheckReq(LinkCheckReqPayload),

    /// Request to send firmware version, LoRaWAN version, and regional parameters version
    #[cmd(cid = 0x7f, len = 0)]
    DutVersionsReq(DutVersionsReqPayload),
}

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum UplinkDUTCommand<'a> {
    /// Returns data sent by EchoIncPayloadReq, where each byte except the initial CID is incremented by 1
    #[cmd(cid = 0x08)]
    EchoIncPayloadAns(EchoIncPayloadAnsPayload<'a>),

    /// Return current RxAppCnt value.
    #[cmd(cid = 0x09, len = 2)]
    RxAppCntAns(RxAppCntAnsPayload<'a>),

    #[cmd(cid = 0x7f, len = 12)]
    DutVersionsAns(DutVersionsAnsPayload<'a>),
}

pub fn parse_downlink_certification_messages(
    data: &[u8],
) -> MacCommandIterator<'_, DownlinkDUTCommand<'_>> {
    MacCommandIterator::new(data)
}

pub fn parse_uplink_certification_messages(
    data: &[u8],
) -> MacCommandIterator<'_, UplinkDUTCommand<'_>> {
    MacCommandIterator::new(data)
}


impl AdrBitChangeReqPayload<'_> {
    /// Enable/disable ADR
    pub fn adr_enable(&self) -> Result<bool, Error> {
        match self.0[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::RFU),
        }
    }
}

impl DutVersionsAnsCreator {
    pub fn set_versions_raw(&mut self, data: [u8; 12]) -> &mut Self {
        self.data[1..=12].copy_from_slice(&data);
        self
    }
}

impl<'a> EchoIncPayloadAnsPayload<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Error> {
        if data.is_empty() {
            return Err(Error::BufferTooShort);
        }
        Ok(EchoIncPayloadAnsPayload(data))
    }

    /// Possible maximum length of the payload not including CID
    const fn max_len() -> usize {
        241
    }

    /// Minimum length of the payload not including CID
    const fn min_len() -> usize {
        1
    }

    /// Actual length of the payload
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        // Payload should have at least minimum length...
        core::cmp::max(Self::min_len(), self.0.len())
    }

    pub fn payload(&self) -> &[u8] {
        self.0
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct EchoIncPayloadAnsCreator {
    pub(crate) data: [u8; EchoIncPayloadAnsPayload::max_len() + 1],
    payload_len: usize,
}

impl Default for EchoIncPayloadAnsCreator {
    fn default() -> Self {
        Self::new()
    }
}

impl EchoIncPayloadAnsCreator {
    pub fn new() -> Self {
        let mut data = [0; EchoIncPayloadAnsPayload::max_len() + 1];
        data[0] = EchoIncPayloadAnsPayload::cid();
        Self { data, payload_len: 0 }
    }
    pub fn build(&self) -> &[u8] {
        &self.data[..=self.payload_len]
    }
    pub const fn cid(&self) -> u8 {
        EchoIncPayloadAnsPayload::cid()
    }
    /// Get the length including CID.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.payload_len + 1
    }

    /// Fill payload and properly mutate this as required.
    pub fn payload(&mut self, data: &[u8]) -> &mut Self {
        self.data[1..=data.len()].iter_mut().zip(data.iter()).for_each(|(dst, &src)| {
            *dst = src.wrapping_add(1);
        });
        self.payload_len = data.len();
        self
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct EchoIncPayloadReqCreator {}
impl UnimplementedCreator for EchoIncPayloadReqCreator {}

impl<'a> EchoIncPayloadReqPayload<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Error> {
        if data.is_empty() {
            return Err(Error::BufferTooShort);
        }
        Ok(EchoIncPayloadReqPayload(data))
    }

    /// Minimum length of the payload not including CID
    const fn min_len() -> usize {
        1
    }

    /// Actual length of the payload
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        // Payload should have at least minimum length...
        core::cmp::max(Self::min_len(), self.0.len())
    }

    /// Return payload
    pub fn payload(&self) -> &[u8] {
        &self.0[0..self.len()]
    }
}

impl RxAppCntAnsCreator {
    pub fn set_rx_app_cnt(&mut self, value: u16) -> &mut Self {
        self.data[1..=2].copy_from_slice(&value.to_le_bytes());
        self
    }
}

impl TxPeriodicityChangeReqPayload<'_> {
    pub fn periodicity(&self) -> Result<Option<u16>, Error> {
        let v = self.0[0];
        if v > 10 {
            Err(Error::RFU)
        } else if v == 0 {
            // DUT should switch back to default behaviour
            Ok(None)
        } else {
            let seconds = match v {
                1 => 5,
                2 => 10,
                3 => 20,
                4 => 30,
                5 => 40,
                6 => 50,
                7 => 60,
                8 => 120,
                9 => 240,
                10 => 480,
                0_u8 | 11_u8..=u8::MAX => unreachable!(),
            };
            Ok(Some(seconds))
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct TxFramesCtrlReqCreator {}
impl UnimplementedCreator for TxFramesCtrlReqCreator {}

impl<'a> TxFramesCtrlReqPayload<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Error> {
        if data.is_empty() {
            return Err(Error::BufferTooShort);
        }
        Ok(TxFramesCtrlReqPayload(data))
    }

    const fn min_len() -> usize {
        2
    }

    /// Actual length of the payload
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        // This payload is without a length field, therefore
        // check whether at least minimum amount of data is present
        // and consume the whole frame until the end.
        core::cmp::max(Self::min_len(), self.0.len())
    }

    /// Whether all subsequent uplinks are overriden as either
    /// L2 Unconfirmed (`FType = 2`) or L2 Confirmed (`FType = 4`)
    pub fn frame_type_override(&self) -> Result<Option<bool>, Error> {
        match self.0[0] {
            // Switch back to device default
            0 => Ok(None),
            // Unconfirmed
            1 => Ok(Some(false)),
            // Confirmed
            2 => Ok(Some(true)),
            _ => Err(Error::RFU),
        }
    }
}
