// Tests for LoRaWAN Certification protocol
use lorawan::protocol::certification::*;
use lorawan::types::Frequency;

mod macros;

#[test]
fn test_txcwreq() {
    let data = [0x08, 0x00, 0xD8, 0xB2, 0x83, 0x0E];
    test_helper!(
        DownlinkDUTCommand,
        data,
        TxCWReq,
        TxCwReqPayload,
        6,
        // 8 seconds
        (timeout, 8u16),
        // 863.1 MHz
        (frequency, Frequency::new_from_raw(&[0xd8, 0xb2, 0x83])),
        // 14 dBM
        (tx_power, 14i8),
    );
}
