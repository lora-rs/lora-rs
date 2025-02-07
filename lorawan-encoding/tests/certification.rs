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

#[test]
fn test_echopayload() {
    let data = [EchoPayloadReqPayload::cid(), 1, 5, 255];
    let mut c = parse_downlink_certification_messages(&data);

    let Some(cmd) = c.next() else { panic!() };
    // Check that whole frame was consumed
    assert_eq!(c.next(), None);

    // Check that all data is present...
    let payload = EchoPayloadReqPayload::new_from_raw(&data[1..]);
    assert_eq!(cmd, EchoPayloadReq(payload));

    // Check that internal payload data actually matches
    let payload = EchoPayloadReqPayload::new(&data[1..]).unwrap();
    assert_eq!(&data[1..], payload.payload());

    // // Check that EchoPayloadAns data transformation works
    // let mut cmd = EchoPayloadAnsCreator::new();
    // assert_eq!(payload.payload().len(), 3);
    // cmd.set_payload(&payload.payload());

    // let cid = EchoPayloadAnsPayload::cid();
    // assert_eq!(cmd.build(), [cid, 2, 6, 0]);
}

#[test]
fn test_echopayloadreq() {
    let data = [EchoPayloadReqPayload::cid(), 1];
    let mut c = parse_downlink_certification_messages(&data);

    if let Some(EchoPayloadReq(payload)) = c.next() {
        assert_eq!(payload.payload(), [1]);
    } else {
        panic!()
    }
}
