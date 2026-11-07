use lora_modulation::{Bandwidth, SpreadingFactor};
use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::types::DR;

use super::{decrypt_uplink, net, util};
use net::{ExchangeAction, Net, Pkt};

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn rxparamsetup_eu868() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // RxParamSetupReq: RX1DRoffset=2, RX2DataRate=DR2 (SF10BW125), Frequency=868525000
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0522c28684"))).await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    assert_eq!(mac.configuration.rx1_dr_offset, 2);
    assert_eq!(mac.configuration.rx2_data_rate, Some(DR::_2));
    assert_eq!(mac.configuration.rx2_frequency, Some(868525000));
    let data = mac.get_session().unwrap().uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [5, 7]);

    // RxComplete (no answer)
    let r = net.exchange(&[1, 2, 3], 4, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    // Validate RX1 data rate (RX1DROffset = 2)
    let rx_conf = r.rx_config(1);
    assert_eq!(rx_conf.rf.bb.sf, SpreadingFactor::_12);
    assert_eq!(rx_conf.rf.bb.bw, Bandwidth::_125KHz);
    // RX2 frequency and DR override
    let rx_conf = r.rx_config(2);
    assert_eq!(rx_conf.rf.frequency, 868525000);
    assert_eq!(rx_conf.rf.bb.sf, SpreadingFactor::_10);
    assert_eq!(rx_conf.rf.bb.bw, Bandwidth::_125KHz);
    let mut uplink = r.uplink(0).clone();
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [5, 7]);

    // RxComplete (no answer), the uplink still carries the answer
    let r = net.exchange(&[1, 2, 3], 4, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    let mut uplink = r.uplink(0).clone();
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [5, 7]);

    // NewChannelReq - add new channel to slot 3
    // LinkADRReq - channelmask in bank = 1, mask = 0b111 (effectively disabling new channel)
    let r = net
        .exchange(&[1, 2, 3], 5, false, ExchangeAction::rx1(Pkt::mac("0703886684500350070001")))
        .await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    // RxParamSetupAns has been dropped...
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [7, 3, 3, 7]);
}
