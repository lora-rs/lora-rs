use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::types::DR;

use super::{net, util};
use net::{ExchangeAction, Net, Pkt};

/// CP-CMD TxFramesCtrlReq padded far beyond what fits an SF12BW125 frame
fn oversized_payload() -> Pkt {
    Pkt::cp_cmd(
        "07020101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101",
    )
}

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn oversized_payload_sf12_bw_125_eu868() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: Send uplink, TCL responds with MAC commands
    // LinkADRReq: DR=0 (SF12BW125), MAX, 0700, 02
    // RXParamSetupReq: Rx1DROffset=0, RX2DataRate=DR0 (SF12BW125), Frequency=869525000
    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("03000700020500d2ad84")))
        .await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    assert_eq!(mac.configuration.rx1_dr_offset, 0);
    assert_eq!(mac.configuration.rx2_data_rate, Some(DR::_0));
    assert_eq!(mac.configuration.rx2_frequency, Some(869525000));
    let data = mac.get_session().unwrap().uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [3, 7, 5, 7]);

    // Step 2: send uplink, TCL responds with an oversized payload in RX1.
    // The LinkADRReq in step 1 set NbTrans to 2, so the oversized (invalid,
    // therefore not acknowledging) downlink triggers a retransmission.
    let r = net.exchange(&[1, 2, 3], 2, false, ExchangeAction::rx1(oversized_payload())).await;
    // We should skip this packet as it's oversized...
    r.expect_rx_complete();
    assert_eq!(r.uplinks().len(), 2);

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    // Only RxParamSetupAns remains
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [5, 7]);

    // Step 3: send regular uplink, TCL responds with the oversized payload
    // in RX2. NbTrans is still 2, so the retransmission's windows time out
    // and the FCntUp is consumed.
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx2(oversized_payload())).await;
    r.expect_rx_complete();
    assert_eq!(r.uplinks().len(), 2);
    // RX1 is not on the RX2 frequency, RX2 is
    assert_ne!(r.rx_config(1).rf.frequency, 869525000);
    assert_eq!(r.rx_config(2).rf.frequency, 869525000);
}
