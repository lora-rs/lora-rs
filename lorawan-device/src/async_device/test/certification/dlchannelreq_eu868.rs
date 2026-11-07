//! LoRaWAN 1.0.4 Certification testcases
//! Based on LoRaWAN 1.0.4 End Device Certification Test Specification v1.6.1
//!
//! DlChannelReq for EU868 region
use super::{decrypt_uplink, net, util};
use net::{ExchangeAction, Net, Pkt};

#[tokio::test]
/// 2.5.3. DlChannelReq test for EU868
/// TODO: Implement test which checks for RX1 frequency
async fn eu868_dlchannelreq() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: send uplink, TCL responds with MAC:DlChannelReq
    // (channel_index=0, frequency=868300000)
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("0a00f87d84"))).await;
    r.expect_downlink_received();

    // Step 2: send uplink, TCL ignores it; the DUT's uplink must carry the
    // DlChannelAns
    let r = net.exchange(&[1, 2, 3], 4, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    // Check that our mac response was present
    let mut uplink = r.uplink(0).clone();
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [0x0a, 0x03]);

    // Step 3: send uplink, TCL ignores it again; the answer must still be
    // present
    let r = net.exchange(&[1, 2, 3], 4, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    // Check that our mac response was present
    let mut uplink = r.uplink(0).clone();
    assert_eq!(decrypt_uplink(&mut uplink).fhdr().f_opts(), [0x0a, 0x03]);
}
