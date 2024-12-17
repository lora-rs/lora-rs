//! LoRaWAN Certification Protocol command and payload handling
use lorawan_macros::CommandHandler;

use crate::maccommands::MacCommandIterator;
use crate::maccommands::SerializableMacCommand;

use crate::maccommands::Error;

use crate::types::DeviceClass;

//#[derive(Debug, PartialEq, CommandHandler)]
//#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[derive(CommandHandler)]
pub enum DownlinkDUTCommand<'a> {
    /// Requests the package version implemented by the end-device
    #[cmd(cid = 0x00, len = 0)]
    PackageVersionReq(PackageVersionReqPayload),

    /// Request to reset the Microcontroller Unit
    #[cmd(cid = 0x01, len = 0)]
    DutResetReq(DutResetReqPayload),

    /// Request to reset the LoRaWAN MAC layer and start issuing Join-Request messages
    #[cmd(cid = 0x02, len = 0)]
    DutJoinReq(DutJoinReqPayload),

    /// Request to change class of operation to A, B, or C
    #[cmd(cid = 0x03, len = 1)]
    SwitchClassReq(SwitchClassReqPayload<'a>),

    /// Request to activate/deactivate Adaptive Data Rate (ADR)
    #[cmd(cid = 0x04, len = 1)]
    AdrBitChangeReq(AdrBitChangeReqPayload<'a>),

    /// Activate/deactivate the regional band duty-cycle enforcement
    #[cmd(cid = 0x05, len = 1)]
    RegionalDutyCycleCtrlReq(RegionalDutyCycleCtrlReqPayload<'a>),

    // TODO
    // /// Change uplink periodicity to the provided value
    // #[cmd(cid = 0x06, len = 1)]
    // TxPeriodicityChangeReq(TxPeriodicityChangeReqPayload),

    // TODO: len = N
    // /// Send all subsequent uplinks of the specified type
    // #[cmd(cid = 0x07, len = ???)]
    // TxFramesCtrlReq(TxFramesCtrlReqPayload),

    // TODO: len = N
    // /// Requests the DUT to echo the provided payload, where each byte is incremented by 1
    // #[cmd(cid = 0x08, len = ???)]
    // EchoPayloadReq(EchoPayloadReqPayload),
    /// Request to provide the current applicative `RxAppCnt` value
    #[cmd(cid = 0x09, len = 0)]
    RxAppCntReq(RxAppCntReqPayload),

    /// Request to reset the applicative `RxAppCnt` value to 0
    #[cmd(cid = 0x0a, len = 0)]
    RxAppCntResetReq(RxAppCntResetReqPayload),

    // 0x0b .. 0x1f RFU
    /// Request to send a `LinkCheckReq` MAC command
    #[cmd(cid = 0x20, len = 0)]
    LinkCheckReq(LinkCheckReqPayload),

    /// Request to send a `DeviceTimeReq` MAC command
    #[cmd(cid = 0x21, len = 0)]
    DeviceTimeReq(DeviceTimeReqPayload),

    // TODO
    // /// Request to send a PingSlotInfoReq MAC command to the TCL; only required for Class B DUTs
    // #[cmd(cid = 0x22, len = 1)]
    // PingSlotInfoReq(PingSlotInfoReqPayload),

    // 0x23 .. 0x3f RFU
    /// Class B: Request to activate/deactivate the autonomous BeaconRxStatusInd transmission
    #[cmd(cid = 0x40, len = 1)]
    BeaconRxStatusIndCtrl(BeaconRxStatusIndCtrlPayload),

    // 0x41 - uplink only

    // TODO
    // /// Class B: Request to provide the current `BeaconCnt` value
    // #[cmd(cid = 0x42, len = 2)]
    // BeaconCntReq(BeaconCntReqPayload),

    // 0x43 - uplink only
    /// Class B: Request to reset the `BeaconCnt` value to 0
    #[cmd(cid = 0x44, len = 0)]
    BeaconCntRstReq(BeaconCntRstReqPayload<'a>),

    // 0x45 .. 0x4f RFU

    // TODO
    // #[cmd(cid = 0x50, len = 4)]
    // SCHCMsgSendReq(SCHCMsgSendReqPayload),

    // 0x51 - uplink only

    // TODO
    // #[cmd(cid = 0x52, len = 1)]
    // FragSessionCntReq(FragSessionCntReqPayload),
    /// Request to activate/deactivate operation in Relay mode
    #[cmd(cid = 0x53, len = 1)]
    RelayModeCtrl(RelayModeCtrlPayload),

    // 0x54 .. 0x7c RFU

    // TODO
    // #[cmd(cid = 0x7d, len = 6)]
    // TxCwReq(TxCwReqPayload),
    /// Request to disable the processing of data received on `FPort=224`
    #[cmd(cid = 0x7e, len = 0)]
    DutFPort224DisableReq(DutFPort224DisableReqPayload),

    /// Request to send firmware version, LoRaWAN version, and regional parameters version
    #[cmd(cid = 0x7f, len = 0)]
    DutVersionsReq(DutVersionsReqPayload),
    // 0x80 .. 0xff Proprietary
}

#[derive(CommandHandler)]
pub enum UplinkDUTCommand {
    // TODO
    // #[cmd(cid = 0x00, len = 2)]
    // PackageVersionAns(PackageVersionAnsPayload),

    // 0x01 .. 0x07 - downlink only

    // TODO
    // #[cmd(cid = 0x08, len = 2)]
    // EchoPayloadAns(EchoPayloadAnsPayload),

    // TODO
    // #[cmd(cid = 0x09, len = 2)]
    // RxAppCntAns(RxAppCntAnsPayload),

    // 0x0a - downlink only
    // 0x0b .. 0x1f RFU
    // 0x20 .. 0x22 downlink only
    // 0x23 .. 0x3f RFU
    // 0x40 - downlink only

    // TODO
    // /// Class B: ...
    // #[cmd(cid = 0x41, len = 22)]
    // BeaconRxStatusInd(BeaconRxStatusIndPayload),

    // 0x42 - downlink only

    // TODO
    // /// Class B: ...
    // #[cmd(cid = 0x43, len = 2)]
    // BeaconCntAns(BeaconCntAnsPayload),

    // 0x44 - downlink only
    // 0x45 .. 0x4f RFU
    /// Answer to the `SCHCMsgSendReq` request
    #[cmd(cid = 0x50, len = 0)]
    SCHCMsgSendAns(SCHCMsgSendAnsPayload),
    // TODO: 48 bytes of header + 1..=60 bytes of data
    // #[cmd(cid = 0x51, len = ...)]
    // SCHCACKInd(SCHCACKIndPayload),

    // TODO
    // #[cmd(cid = 0x52, len = 3)]
    // FragSessionCntAns(FragSessionCntAnsPayload),

    // 0x53 - downlink only
    // 0x54 .. 0x7c RFU
    // 0x7d - 0x7e - downlink only

    // TODO
    // #[cmd(cid = 0x7f, len = 12)]
    // DutVersionsAns(DutVersionsAnsPayload),

    // 0x80 .. 0xff Proprietary
}

impl AdrBitChangeReqPayload<'_> {
    /// Enable/disable ADR
    pub fn adr_enable(&self) -> bool {
        self.0[0] == 0x01
    }
}

impl BeaconCntRstReqPayload<'_> {
    /// Enable/disable `BeaconRxStatusInd` transmission
    pub fn ctrl_enable(&self) -> bool {
        self.0[0] == 0x01
    }
}

impl RegionalDutyCycleCtrlReqPayload<'_> {
    /// Enable/disable regional duty-cycle enforcement
    pub fn dutycycle_enable(&self) -> bool {
        self.0[0] == 0x01
    }
}

impl RegionalDutyCycleCtrlReqPayload<'_> {
    /// Enable/disable regional duty-cycle enforcement
    pub fn ctrl_enable(&self) -> bool {
        self.0[0] == 0x01
    }
}

impl SwitchClassReqPayload<'_> {
    /// Return requested device class
    pub fn class(&self) -> Result<DeviceClass, Error> {
        DeviceClass::try_from(self.0[0])
    }
}

impl SwitchClassReqCreator {
    pub fn set_class(&mut self, device_class: DeviceClass) -> &mut Self {
        self.data[1] = device_class.into();

        self
    }
}
