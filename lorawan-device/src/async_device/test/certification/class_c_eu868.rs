//! LoRaWAN 1.0.4 Class C certification testcases for EU868.
//!
use super::{net, util};
use net::{ExchangeAction, Net, Pkt};

/// Based on TP_C_EU868_ED_MAC_104_BV_000 - Testing for downlinks of an
/// Unconfirmed frame on RX1 and a Confirmed frame on RXC (2-3) window.
///
/// Step 19 sets a 20 s uplink periodicity and Step 20 sends a confirmed
/// TxFramesCtrlReq in RXC. The DUT must acknowledge it immediately, not with
/// the next periodic uplink.
#[cfg(feature = "class-c")]
#[tokio::test]
async fn eu868_class_c_confirmed_downlink_in_rxc_is_acknowledged() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;

    let action = ExchangeAction::none()
        .in_window(1, Pkt::cp_cmd("0603"))
        .rxc_after(1, Pkt::cp_cmd("0700").confirmed());
    let r = net.exchange(&[1, 2, 3], 3, false, action).await;
    r.expect_rx_complete();
    assert_eq!(net.device().get_override_periodicity(), Some(20));

    assert_eq!(r.uplinks().len(), 2, "confirmed downlink must be acknowledged");
    let mut uplink = r.uplink(1).clone();
    let dl = util::decrypt_uplink(&mut uplink);
    assert!(dl.fhdr().fctrl().ack(), "ACK bit must be set");
    assert!(!dl.is_confirmed(), "the ACK is an unconfirmed uplink");
    assert_eq!(dl.f_port(), None, "an ACK-only uplink has no FPort");
}

/// A confirmed certification command that produces no application response
/// (such as `TxPeriodicityChangeReq`) received during `retransmit_wait`
/// must still transmit its immediate ACK rather than leaving it stashed.
#[cfg(feature = "class-c")]
#[tokio::test]
async fn eu868_class_c_confirmed_tx_periodicity_in_retransmit_wait_is_acked() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;
    net.exchange(&[1], 1, false, ExchangeAction::none()).await.expect_rx_complete();

    let action = ExchangeAction::none().rxc_after(2, Pkt::cp_cmd("0603").confirmed());
    let r = net.exchange(&[1, 2, 3], 3, true, action).await;
    assert_eq!(net.device().get_override_periodicity(), Some(20));

    assert_eq!(r.uplinks().len(), 2, "confirmed TxPeriodicityChangeReq must be acknowledged");
    let mut ack = r.uplink(1).clone();
    let dl = util::decrypt_uplink(&mut ack);
    assert!(dl.fhdr().fctrl().ack(), "ACK bit must be set");
    assert!(!dl.is_confirmed(), "the ACK is an unconfirmed uplink");
    assert_eq!(dl.f_port(), None, "an ACK-only uplink has no FPort");
}
