use super::*;
use crate::{
    radio::{RxQuality, TxConfig},
    region,
    test_util::*,
};
use lorawan::maccommands::{UplinkMacCommand, parse_uplink_mac_commands};

mod timer;
use timer::TestTimer;

mod radio;
use radio::TestRadio;

mod net;
use net::{ExchangeAction, Net, Pkt};

mod util;
pub(crate) use util::{setup, setup_with_session};

#[cfg(feature = "certification")]
mod certification;

mod maccommands;

mod nvm;

#[cfg(feature = "class-c")]
mod class_c;

#[cfg(feature = "multicast")]
mod multicast;

type Device<S = crate::nvm::NoNvm, G = rand_core::OsRng> =
    crate::async_device::Device<TestRadio, TestTimer, G, 512, 4, S>;

#[tokio::test]
async fn test_join_rx1() {
    let (radio, timer, device) = setup();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.join(ExchangeAction::rx1(Pkt::join_accept())).await;
    r.expect_joined();
    util::assert_join_request(r.uplink(0));
    assert_eq!(r.windows(), 1);
}

#[tokio::test]
async fn test_join_rx2() {
    let (radio, timer, device) = setup();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.join(ExchangeAction::rx2(Pkt::join_accept())).await;
    r.expect_joined();
    assert_eq!(r.windows(), 2);
}

#[tokio::test]
async fn test_no_join_accept() {
    let (radio, timer, device) = setup();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.join(ExchangeAction::none()).await;
    r.expect_no_join_accept();
    assert_eq!(r.windows(), 2);
}

#[tokio::test]
async fn test_join_accept_dl_settings_applied() {
    let (radio, timer, device) = setup();
    let mut net = Net::new(radio, timer, device).await;
    // JoinAccept with RX1DROffset = 2 and RX2DataRate = DR10 (both valid for US915)
    let r = net.join(ExchangeAction::rx1(Pkt::join_accept().dl_settings(0x2A))).await;
    r.expect_joined();
    assert_eq!(net.device().mac.configuration.rx1_dr_offset, 2);
    assert_eq!(net.device().mac.configuration.rx2_data_rate, Some(region::DR::_10));
}

#[tokio::test]
async fn test_join_accept_dl_settings_invalid_values_ignored() {
    let (radio, timer, device) = setup();
    let mut net = Net::new(radio, timer, device).await;
    // RX1DROffset = 7 (US915 max is 3) and RX2DataRate = DR7 (RFU in US915)
    let r = net.join(ExchangeAction::rx1(Pkt::join_accept().dl_settings(0x77))).await;
    r.expect_joined();
    // Invalid values are ignored; defaults remain
    assert_eq!(net.device().mac.configuration.rx1_dr_offset, 0);
    assert_eq!(net.device().mac.configuration.rx2_data_rate, None);
}

#[tokio::test]
async fn test_join_accept_bad_mic_changes_nothing() {
    let (radio, timer, device) = setup();
    let rx1_delay_default = device.mac.configuration.rx1_delay;
    let mut net = Net::new(radio, timer, device).await;
    // JoinAccept built with the wrong key; carries RxDelay = 3 and DLSettings = 0x2A
    let accept = Pkt::join_accept().bad_mic().dl_settings(0x2A).rx_delay(3);
    let r = net.join(ExchangeAction::rx1(accept)).await;
    r.expect_no_join_accept();
    assert_eq!(r.windows(), 2);
    // The rejected accept must not have touched the configuration
    let config = &net.device().mac.configuration;
    assert_eq!(config.rx1_delay, rx1_delay_default);
    assert_eq!(config.rx1_dr_offset, 0);
    assert_eq!(config.rx2_data_rate, None);
}

#[tokio::test]
async fn test_unconfirmed_uplink_no_downlink() {
    let (radio, timer, device) = setup_with_session();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    // send returns only once both windows have closed
    assert_eq!(r.windows(), 2);
}

/// What the stack does when the radio reports a receive error during RX1:
/// with lora-phy PR 487 that is a downlink whose header CRC failed. Every
/// PhyError from rx_single ends the send, so RX2 never opens.
#[tokio::test]
async fn test_unconfirmed_uplink_phy_error_in_rx1_ends_the_send() {
    let (radio, timer, device) = setup_with_session();
    let mut net = Net::new(radio, timer, device).await;
    let r = net
        .exchange(&[1, 2, 3], 3, false, ExchangeAction::none().phy_error(1, "HeaderError"))
        .await;
    assert!(
        matches!(r.response(), Err(Error::Radio("HeaderError"))),
        "send() after a bad frame in RX1: {:?}",
        r.response()
    );
    // No RX2 window was opened
    assert_eq!(r.windows(), 1);
}

#[tokio::test]
async fn test_unconfirmed_uplink_retransmission() {
    let (radio, timer, mut device) = setup_with_session();
    device.mac.configuration.nb_trans = 2;
    let mut net = Net::new(radio, timer, device).await;

    // Both transmissions: RX1 and RX2 time out
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::none()).await;
    r.expect_rx_complete();
    assert_eq!(r.uplinks().len(), 2);
    // The retransmission resends the exact same frame (same FCntUp)
    assert_eq!(r.uplink(0).data(), r.uplink(1).data());
    // The FCntUp was consumed only once for the two attempts
    assert_eq!(net.device().get_session().unwrap().fcnt_up, 1);
}

#[tokio::test]
async fn test_unconfirmed_uplink_downlink_stops_retransmission() {
    let (radio, timer, mut device) = setup_with_session();
    device.mac.configuration.nb_trans = 3;
    let mut net = Net::new(radio, timer, device).await;

    // First transmission: RX1 times out, a downlink arrives in RX2
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx2(Pkt::app(4, &[3, 2, 1]))).await;
    r.expect_downlink_received();
    // A single transmission was made
    assert_eq!(r.uplinks().len(), 1);
    assert_eq!(net.device().get_session().unwrap().fcnt_up, 1);
}

#[tokio::test]
async fn test_confirmed_uplink_no_ack_retransmission() {
    let (radio, timer, mut device) = setup_with_session();
    device.mac.configuration.nb_trans = 3;
    let mut net = Net::new(radio, timer, device).await;

    // Three copies, each followed by a RETRANSMIT_TIMEOUT wait
    let r = net.exchange(&[1, 2, 3], 3, true, ExchangeAction::none()).await;
    r.expect_no_ack();
    assert_eq!(r.uplinks().len(), 3);
    assert_eq!(net.device().get_session().unwrap().fcnt_up, 1);
}

#[tokio::test]
async fn test_confirmed_uplink_no_ack() {
    let (radio, timer, device) = setup_with_session();
    let mut net = Net::new(radio, timer, device).await;
    // Both windows time out, then the RETRANSMIT_TIMEOUT runs out
    let r = net.exchange(&[1, 2, 3], 3, true, ExchangeAction::none()).await;
    r.expect_no_ack();
    assert_eq!(r.windows(), 2);
}

#[tokio::test]
async fn test_confirmed_uplink_with_ack_rx1() {
    let (radio, timer, device) = setup_with_session();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.exchange(&[1, 2, 3], 3, true, ExchangeAction::rx1(Pkt::empty().ack())).await;
    r.expect_downlink_received();
    assert_eq!(r.windows(), 1);
}

#[tokio::test]
async fn test_confirmed_uplink_with_ack_rx2() {
    let (radio, timer, device) = setup_with_session();
    let mut net = Net::new(radio, timer, device).await;
    let r = net.exchange(&[1, 2, 3], 3, true, ExchangeAction::rx2(Pkt::empty().ack())).await;
    r.expect_downlink_received();
    assert_eq!(r.windows(), 2);
}

#[tokio::test]
async fn test_link_adr_ans() {
    let (radio, timer, device) = setup_with_session();
    let mut net = Net::new(radio, timer, device).await;

    // Two LinkADRReq (DR0, max power, ChMaskCntl 5 enabling bank 1 then
    // bank 2) piggybacked on an application downlink
    let downlink = Pkt::app(4, &[3, 2, 1]).fopts("03000200500300040050");
    let r = net.exchange(&[1, 2, 3], 3, true, ExchangeAction::rx1(downlink)).await;
    r.expect_downlink_received();

    // The next uplink carries both LinkADRAns
    let r = net.exchange(&[1, 2, 3], 3, true, ExchangeAction::rx1(Pkt::empty().ack())).await;
    r.expect_downlink_received();
    let mut uplink = r.uplink(0).clone();
    let answers: std::vec::Vec<UplinkMacCommand<'_>> =
        parse_uplink_mac_commands(util::decrypt_uplink(&mut uplink).fhdr().f_opts())
            .collect::<Result<_, _>>()
            .unwrap();
    assert_eq!(answers.len(), 2);
    assert!(answers.iter().all(|ans| matches!(ans, UplinkMacCommand::LinkADRAns(_))));
}

#[tokio::test]
async fn invalid_maccommands_in_frmpayload() {
    let (radio, timer, device) = setup_with_session();
    let mut net = Net::new(radio, timer, device).await;
    // A LinkADRReq whose ChMaskCntl (6) is invalid for US915, received in RX2
    let r = net.exchange(&[1, 2, 3], 3, false, ExchangeAction::rx2(Pkt::mac("03c0000000"))).await;
    r.expect_downlink_received();
    // Nothing in downlink as expected
    assert!(net.device().take_downlink().is_none());
}
