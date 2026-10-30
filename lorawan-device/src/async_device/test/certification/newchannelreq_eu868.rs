//! LoRaWAN 1.0.4 Certification testcases
//!
//! NewChannelReq for EU868 (2.5.2)
//! * Add/remove read-only default channels
//! * Add/remove single channel
//! * TODO: Invalid frequency/datarate tests
use super::{net, util};
use net::{ExchangeAction, Net, Pkt};

use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::types::DR;

#[tokio::test]
/// NewChannelReq: Add/remove read-only default channels
async fn newchannelreq_readonly_default_eu868() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: attempt to add/modify read-only channels
    let r = net
        .exchange(
            &[1, 2, 3],
            1,
            false,
            ExchangeAction::rx1(Pkt::mac("0700184f84500701184f84500702184f8450")),
        )
        .await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 3);
    assert_eq!(data, [0x07, 0x00, 0x07, 0x00, 0x07, 0x00]);

    // Step 2: attempt to remove read-only channels
    let r = net
        .exchange(
            &[1, 2, 3],
            2,
            false,
            ExchangeAction::rx1(Pkt::mac("070000000050070100000050070200000050")),
        )
        .await;
    r.expect_downlink_received();

    let session = net.device().mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 3);
    assert_eq!(data, [0x07, 0x00, 0x07, 0x00, 0x07, 0x00]);
}

#[tokio::test]
/// NewChannelReq: Addition/removal of single channel
async fn newchannelreq_e868() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: send uplink, TCL responds with LinkADRReq
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("0350070001"))).await;
    r.expect_downlink_received();

    // Step 2: NewChannelReq(ChIndex=0f, Freq=867700000, DrRange=50); check
    // whether DevStatusAns is present in MAC
    let r = net.exchange(&[1, 2, 3], 2, false, ExchangeAction::rx1(Pkt::mac("070f88668450"))).await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [0x07, 0x03]);
    // Check whether channel was added
    let channel_mask = mac.region.channel_mask_get();
    assert_eq!(channel_mask.is_enabled(0xf), Ok(true));

    // Step 3: NewChannelReq(ChIndex=0f, Freq=0, DrRange=50)
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx1(Pkt::mac("070f00000050"))).await;
    r.expect_downlink_received();

    let mac = &net.device().mac;
    let session = mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [0x07, 0x03]);
    // Check whether channel was removed (channel disabled in ChannelMask)
    let channel_mask = mac.region.channel_mask_get();
    assert_eq!(channel_mask.is_enabled(0xf), Ok(false));
}

#[tokio::test]
#[should_panic(expected = "DR5-only channel 3 selected at DR2")]
/// LCTT TP_A_EU868_ED_MAC_104_BV_006, "Checking channel usage 12 to 15":
/// a channel whose data-rate range excludes the current DR must not be selected.
async fn dr_restricted_channel_not_selected_eu868() {
    let (radio, timer, device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut net = Net::new(radio, timer, device).await;

    // Step 1: add channel 3 (867.1 MHz) with DR5 only.
    let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::rx1(Pkt::mac("0703184f8455"))).await;
    r.expect_downlink_received();
    assert!(net.device().mac.region.channel_mask_get().is_enabled(3).unwrap());

    // Step 2: switch to DR2 with ChMask FFFF.
    let r = net.exchange(&[1, 2, 3], 2, false, ExchangeAction::rx1(Pkt::mac("0320ffff01"))).await;
    r.expect_downlink_received();
    assert_eq!(net.device().mac.configuration.data_rate, DR::_2);

    // Step 3: channel 3 only supports DR5, so it must never be selected at DR2.
    for _ in 0..40 {
        let r = net.exchange(&[1, 2, 3], 1, false, ExchangeAction::none()).await;
        r.expect_rx_complete();

        let frequency = r.uplink(0).tx_config().rf.frequency;
        assert!(
            matches!(frequency, 868_100_000 | 868_300_000 | 868_500_000),
            "DR5-only channel 3 selected at DR2 ({frequency} Hz)"
        );
    }
}
