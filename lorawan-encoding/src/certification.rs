//! LoRaWAN Certification Protocol (TS009) command and payload handling
use lorawan_macros::CommandHandler;

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
}

pub fn parse_downlink_certification_messages(
    data: &[u8],
) -> MacCommandIterator<'_, DownlinkDUTCommand<'_>> {
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
