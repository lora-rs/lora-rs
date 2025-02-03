//! LoRaWAN Certification Protocol (TS009) command and payload handling
use lorawan_macros::CommandHandler;

use crate::maccommands::MacCommandIterator;
use crate::maccommands::SerializableMacCommand;

#[derive(Debug, PartialEq, CommandHandler)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum DownlinkDUTCommand {
    /// Request to reset the Microcontroller Unit
    #[cmd(cid = 0x01, len = 0)]
    DutResetReq(DutResetReqPayload),
}

pub fn parse_downlink_certification_messages(
    data: &[u8],
) -> MacCommandIterator<'_, DownlinkDUTCommand> {
    MacCommandIterator::new(data)
}
