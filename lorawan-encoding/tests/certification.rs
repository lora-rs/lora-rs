use lorawan::certification::parse_downlink_dut_commands;
use lorawan::certification::DownlinkDUTCommand::*;
use lorawan::certification::*;
use lorawan::maccommands::ParseError as Error;

#[test]
fn test_parse_empty_downlink() {
    assert_eq!(parse_downlink_dut_commands(&[]).count(), 0);
}

#[test]
fn test_parse_variable_txframectrlreq() {
    // A lone CID for a variable-length command is a truncation error.
    let mut c = parse_downlink_dut_commands(&[0x07]);
    assert_eq!(c.next(), Some(Err(Error::Truncated { cid: 0x07 })));
    assert_eq!(c.next(), None);

    // A single FrameType octet is a complete TxFramesCtrlReq.
    assert_eq!(parse_downlink_dut_commands(&[0x07, 0x02]).map(Result::unwrap).count(), 1);
    assert_eq!(
        parse_downlink_dut_commands(&[0x07, 0x02, 0x02, 0x04]).map(Result::unwrap).count(),
        1
    );

    let mut c = parse_downlink_dut_commands(&[0x07, 0x02, 0x03]);
    assert_eq!(c.next(), Some(Ok(TxFramesCtrlReq(TxFramesCtrlReqPayload::new(&[2, 3]).unwrap()))));

    let data = [0x07, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
    let mut c = parse_downlink_dut_commands(&data);
    // Make sure whole buffer is consumed as single payload...
    assert_eq!(
        c.next(),
        Some(Ok(TxFramesCtrlReq(TxFramesCtrlReqPayload::new(&data[1..]).unwrap())))
    );
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
    let data = [EchoIncPayloadReqPayload::cid(), 1, 5, 255];
    let mut c = parse_downlink_dut_commands(&data);

    let Some(Ok(cmd)) = c.next() else { panic!() };
    // Check that whole frame was consumed
    assert_eq!(c.next(), None);

    // Check that all data is present...
    let payload = EchoIncPayloadReqPayload::new_from_raw(&data[1..]);
    assert_eq!(cmd, EchoIncPayloadReq(payload));

    // Check that internal payload data actually matches
    let payload = EchoIncPayloadReqPayload::new(&data[1..]).unwrap();
    assert_eq!(&data[1..], payload.payload());

    let mut cmd = EchoIncPayloadAnsCreator::new();
    assert_eq!(cmd.build(), [EchoIncPayloadAnsPayload::cid()]);

    // Push in data...
    cmd.payload(&data[1..]);

    // ...and check whether this has been properly mutated
    let out = cmd.build();
    assert_eq!(out.len(), 4);
    assert_eq!(out[1..], [2, 6, 0]);

    cmd.payload(&[]);
    assert_eq!(cmd.build().len(), 1);
}

#[test]
fn test_echopayloadreq() {
    let data = [EchoIncPayloadReqPayload::cid(), 1];
    let mut c = parse_downlink_dut_commands(&data);

    if let Some(Ok(EchoIncPayloadReq(payload))) = c.next() {
        assert_eq!(payload.payload(), [1]);
    } else {
        panic!()
    }
}
