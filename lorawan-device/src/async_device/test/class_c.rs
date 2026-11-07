use super::net::{ExchangeAction, Pkt};
use super::util;

/// An application downlink on FPort 3, as the network sends it in RXC.
fn class_c_downlink() -> Pkt {
    Pkt::app(3, &[1, 2, 3])
}

#[tokio::test]
async fn test_class_c_data_before_rx1() {
    let mut net = util::setup_with_session_class_c().await;
    // A downlink in RXC between the transmission and RX1, then an
    // application downlink in RX1
    let action = ExchangeAction::rx1(Pkt::app(4, &[3, 2, 1])).rxc_after(0, class_c_downlink());
    let r = net.exchange(&[1, 2, 3], 3, true, action).await;
    r.expect_downlink_received();
    // The class C setup consumed FCntUp 0
    assert_eq!(util::uplink_fcnt(r.uplink(0)), 1);
    // Both downlinks reached the application
    let device = net.device();
    let _ = device.take_downlink().unwrap();
    let _ = device.take_downlink().unwrap();
    assert!(device.take_downlink().is_none());
}

#[tokio::test]
async fn test_class_c_data_before_rx2() {
    let mut net = util::setup_with_session_class_c().await;
    // RX1 times out, a downlink in RXC before RX2, then an application
    // downlink in RX2
    let action = ExchangeAction::rx2(Pkt::app(4, &[3, 2, 1])).rxc_after(1, class_c_downlink());
    let r = net.exchange(&[1, 2, 3], 3, true, action).await;
    r.expect_downlink_received();
    assert_eq!(util::uplink_fcnt(r.uplink(0)), 1);
    let device = net.device();
    let _ = device.take_downlink().unwrap();
    let _ = device.take_downlink().unwrap();
    assert!(device.take_downlink().is_none());
}

#[tokio::test]
async fn test_class_c_downlink_during_retransmit_timeout() {
    let mut net = util::setup_with_session_class_c().await;
    // No downlink in RX1 or RX2; class C keeps listening in RXC during the
    // RETRANSMIT_TIMEOUT.
    let action = ExchangeAction::none().rxc_after(2, class_c_downlink());
    let r = net.exchange(&[1, 2, 3], 3, true, action).await;
    // The accepted downlink ends the uplink cycle, as it would in RX1/RX2
    r.expect_downlink_received();
    assert_eq!(r.fcnt_down(), Some(1));
    let downlink = net.device().take_downlink().unwrap();
    assert_eq!(downlink.data, [1, 2, 3]);
    assert_eq!(downlink.fport, 3);
}

#[tokio::test]
async fn test_class_c_downlink_during_confirmed_repetition_wait() {
    let mut net = util::setup_with_session_class_c().await;
    net.device().mac.configuration.nb_trans = 3;
    // No downlink in RX1 or RX2 of the first copy; a downlink in RXC during
    // the RETRANSMIT_TIMEOUT before the second copy ends the retransmission
    // cycle: no second copy is sent.
    let action = ExchangeAction::none().rxc_after(2, class_c_downlink());
    let r = net.exchange(&[1, 2, 3], 3, true, action).await;
    r.expect_downlink_received();
    assert_eq!(r.uplinks().len(), 1);
    let device = net.device();
    let downlink = device.take_downlink().unwrap();
    assert_eq!(downlink.data, [1, 2, 3]);
    assert_eq!(device.get_session().unwrap().fcnt_up, 2);
}

#[tokio::test]
async fn test_class_c_async_down() {
    let mut net = util::setup_with_session_class_c().await;
    let r = net.listen(vec![class_c_downlink()]).await;
    r.expect_downlink_received();
    let _ = net.device().take_downlink().unwrap();
}
