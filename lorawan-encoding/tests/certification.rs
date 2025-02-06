use lorawan::certification::parse_downlink_certification_messages;
use lorawan::certification::DownlinkDUTCommand::*;
use lorawan::certification::*;

#[test]
fn test_parse_empty_downlink() {
    assert_eq!(parse_downlink_certification_messages(&[]).count(), 0);
}

#[test]
fn test_parse_variable_txframectrlreq() {
    assert_eq!(parse_downlink_certification_messages(&[0x07]).count(), 0);
    assert_eq!(parse_downlink_certification_messages(&[0x07, 0x02]).count(), 1);
    assert_eq!(parse_downlink_certification_messages(&[0x07, 0x02, 0x02, 0x04]).count(), 1);

    let mut c = parse_downlink_certification_messages(&[0x07, 0x02, 0x03]);
    assert_eq!(c.next(), Some(TxFramesCtrlReq(TxFramesCtrlReqPayload::new(&[2, 3]).unwrap())));

    let data = [0x07, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
    let mut c = parse_downlink_certification_messages(&data);
    // Make sure whole buffer is consumed as single payload...
    assert_eq!(c.next(), Some(TxFramesCtrlReq(TxFramesCtrlReqPayload::new(&data[1..]).unwrap())));
    // ..end there's nothing left
    assert_eq!(c.next(), None);
}

#[test]
fn test_dutversionsans() {
    let mut cmd = DutVersionsAnsCreator::new();
    let cid = DutVersionsAnsPayload::cid();
    cmd.set_versions_raw([
        0, 0, 0, 1, // Firmware version
        1, 0, 4, 0, // Lorawan version - 1.0.4
        2, 1, 0, 4, // region version, RP002-1.0.4 == 2.1.0.4
    ]);

    assert_eq!(cmd.build(), [cid, 0, 0, 0, 1, 1, 0, 4, 0, 2, 1, 0, 4]);
}
