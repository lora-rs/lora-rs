use super::net::{ExchangeAction, Net, Pkt};
use super::util;

use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::types::ChannelMask;

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn linkadrreq_dynamic() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // NewChannelReq - add new channel to slot 3
    // LinkADRReq - channelmask in bank = 1, mask = 0b111 (effectively disabling new channel)
    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0703886684500350070001")))
        .await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [7, 3, 3, 7]);

    // Trigger second uplink which calls "disable_all_channels" LinkADRReq and
    // our MAC layer effectively does it... This is wrong.
    // LinkADRReq - disable ALL channels in bank 0
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0350000001"))).await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn linkadrreq_fixed_125khz_extra_mask() {
    let (radio, timer, device) = util::session_with_region(crate::region::US915::default().into());
    let mut net = Net::new(radio, timer, device).await;

    // LinkADRReq, SF8BW500, MAX, 0100, 71
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0340010071"))).await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 7]);
    // Make sure that extra mask is properly applied to bank 8
    assert_eq!(
        mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap()
    );
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn linkaddreq_fixed_channel_mask_validation() {
    let (radio, timer, device) = util::session_with_region(crate::region::US915::default().into());
    let mut net = Net::new(radio, timer, device).await;

    // LinkADRReq, SF8BW500, MAX, 0000, 61: enable 125 kHz, disable 500 kHz
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0340000061"))).await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);

    // LinkADRReq, SF8BW500, MAX, 0100, 71: disable 125 kHz, switch to 500 kHz
    let r = net.exchange(&[1, 2, 3], 4, false, ExchangeAction::rx1(Pkt::mac("0340010071"))).await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 7]);
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn linkaddreq_fixed_invalid_125khz() {
    // 2.5.8.b.ii.1.2. Invalid Command Processing
    let (radio, timer, device) = util::session_with_region(crate::region::US915::default().into());
    let mut net = Net::new(radio, timer, device).await;

    // step 1
    let r = net
        .exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("03000000710300ff0001")))
        .await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [3, 7, 3, 7]);
    assert_eq!(
        mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0xff, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap()
    );

    // step 2
    let r = net.exchange(&[1, 2, 3], 2, false, ExchangeAction::rx1(Pkt::mac("0340000061"))).await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);
    assert_eq!(
        mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0xff, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap()
    );

    // step 3
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0340010071"))).await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 7]);
    assert_eq!(
        mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap()
    );

    // step 4
    // Invalid LinkADRReq enables one 125kHz channel with allowed 125kHz datarate
    let r = net.exchange(&[1, 2, 3], 4, false, ExchangeAction::rx1(Pkt::mac("0300000101"))).await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [3, 6]);
    assert_eq!(
        mac.region.channel_mask_get(),
        ChannelMask::<9>::new(&[0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap()
    );
}

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn linkadrreq_dynamic_invalid() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // LinkADRReq, SF8BW125, 4, 0100, 00
    // LinkADRReq, SF9BW125, 1, 0000, 61
    // LinkADRReq, SF7BW125, 0, 0000, 01
    let r = net
        .exchange(
            &[1, 2, 3],
            3,
            false,
            ExchangeAction::rx1(Pkt::mac("034401000003310000610350000001")),
        )
        .await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 3);
    assert_eq!(data, [3, 6, 3, 6, 3, 6]);
}

/// NewChannelReq for the first 3 channels: in EU868 those are join
/// channels and read-only
fn newchannelreq_invalid_eu868() -> Pkt {
    // NewChannelReqPayload([0, 24, 79, 132, 80])
    // NewChannelReqPayload([1, 24, 79, 132, 80])
    // NewChannelReqPayload([2, 24, 79, 132, 80])
    Pkt::mac("0700184f84500701184f84500702184f8450")
}

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn newchannelreq_eu868_readonly() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(newchannelreq_invalid_eu868()))
        .await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().expect("Session not joined?");
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 3);
    // For all 3 channels freq and dr are nacked (0b11)
    assert_eq!(data, [7, 0, 7, 0, 7, 0]);
}

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn newchannelreq_eu868_invalid_dr() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // NewChannelReq with invalid DataRateRange
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("0703287684cd"))).await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().expect("Session not joined?");
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    // Frequency is acked (bit 0), dr is invalid (bit 1)
    assert_eq!(data, [7, 0b01]);
}

#[tokio::test]
#[cfg(feature = "region-us915")]
async fn newchannelreq_fixed_region_ignore() {
    let (radio, timer, device) = util::session_with_region(crate::region::US915::default().into());
    let mut net = Net::new(radio, timer, device).await;

    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(newchannelreq_invalid_eu868()))
        .await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().expect("Session not joined?");
    let data = session.uplink.mac_commands();
    // Fixed channel region ignores NewChannelReq commands
    assert_eq!(parse_uplink_mac_commands(data).count(), 0);
}

#[tokio::test]
#[cfg(all(feature = "region-us915", feature = "experimental"))]
// TODO: Finalize RXParamSetupReq/RXTimingSetupReq
async fn maccommands_in_frmpayload() {
    let (radio, timer, device) = util::setup_with_session();
    let mut net = Net::new(radio, timer, device).await;

    // FRMPayload contains:
    // - DevStatusReq(..)
    // - RXParamSetupReq(RXParamSetupReqPayload([2, 210, 173, 132])) - freq: 869525000
    // - RXTimingSetupReq(RXTimingSetupReqPayload([1]))
    // - LinkADRReq(LinkADRReqPayload([80, 0, 0, 97]))
    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("060502d2ad8408010350000061")))
        .await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().expect("Session not joined?");
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 4);
    // LinkADRReq sends freq = 869525000, but this is invalid in US915
    assert_eq!(mac.configuration.rx2_frequency, None);
    // TODO: Implement RxParamSetup and RxTimingSetup...
    // assert_eq!(data, [5, 7]);
}
