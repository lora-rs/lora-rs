use super::util;
use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::{get_key, Uplink};

use lorawan::default_crypto::DefaultFactory;
use lorawan::maccommands::parse_uplink_mac_commands;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
// TODO: Finalize RXParamSetupReq
async fn maccommands_in_frmpayload() {
    fn frmpayload_maccommands(
        _uplink: Option<Uplink>,
        _config: RfConfig,
        rx_buffer: &mut [u8],
    ) -> usize {
        // FRMPayload contains:
        // - DevStatusReq(..)
        // - RXParamSetupReq(RXParamSetupReqPayload([2, 210, 173, 132]))
        // - RXTimingSetupReq(RXTimingSetupReqPayload([1]))
        // - LinkADRReq(LinkADRReqPayload([80, 0, 0, 97]))
        let mut phy = lorawan::creator::DataPayloadCreator::new(rx_buffer).unwrap();
        phy.set_confirmed(false);
        phy.set_f_port(0);
        phy.set_dev_addr(&[0; 4]);
        phy.set_uplink(false);
        phy.set_fcnt(5);
        phy.set_fctrl(&lorawan::parser::FCtrl::new(0x00, true));
        let finished = phy
            .build(
                &[],
                [6, 5, 2, 0xd2, 0xad, 0x84, 8, 1, 3, 0x50, 0, 0, 0x61],
                &get_key().into(),
                &get_key().into(),
                &DefaultFactory,
            )
            .unwrap();
        finished.len()
    }

    let (radio, timer, mut async_device) = util::setup_with_session();
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });

    // Handle reception in RX1
    timer.fire_most_recent().await;

    radio.handle_rxtx(frmpayload_maccommands).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(5)) => {}
        _ => panic!(),
    }

    if let Some(session) = device.mac.get_session() {
        let data = session.uplink.mac_commands();
        assert_eq!(parse_uplink_mac_commands(data).count(), 4);
        assert_eq!(device.mac.configuration.rx2_frequency, Some(869525000));
    } else {
        panic!("Session not joined?");
    }
}
