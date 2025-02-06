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

    /// Change uplink periodicity to the provided value
    #[cmd(cid = 0x06, len = 1)]
    TxPeriodicityChangeReq(TxPeriodicityChangeReqPayload<'a>),
}

pub fn parse_downlink_certification_messages(
    data: &[u8],
) -> MacCommandIterator<'_, DownlinkDUTCommand<'_>> {
    MacCommandIterator::new(data)
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
