use crate::async_device::SendResponse;
use crate::radio::RfConfig;
use crate::test_util::Uplink;

use lora_modulation::{Bandwidth, SpreadingFactor};
use lorawan::maccommands::parse_uplink_mac_commands;
use lorawan::parser::{DataHeader, DataPayload, PhyPayload};
use lorawan::types::DR;

use super::{build_mac, util};

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
#[cfg(feature = "region-eu868")]
async fn rxparamsetup_eu868() {
    let (radio, timer, mut async_device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let send_await_complete = Arc::new(Mutex::new(false));

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = async_device.send(&[1, 2, 3], 3, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (async_device, response)
    });

    fn rxparamsetup_1(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // RxParamSetupReq: RX1DRoffset=2, RX2DataRate=DR2 (SF10BW125), Frequency=868525000
        build_mac(buf, "0522c28684", 1)
    }

    timer.fire_most_recent().await;
    radio.handle_rxtx(rxparamsetup_1).await;

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    assert_eq!(device.mac.configuration.rx1_dr_offset, 2);
    assert_eq!(device.mac.configuration.rx2_data_rate, Some(DR::_2));
    assert_eq!(device.mac.configuration.rx2_frequency, Some(868525000));

    let data = session.uplink.mac_commands();
    assert_eq!(parse_uplink_mac_commands(data).count(), 1);
    assert_eq!(data, [5, 7]);

    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 4, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    // RX1
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    // Validate RX1 data rate (RX1DROffset = 2)
    let rx_conf = radio.get_rxconfig().await.unwrap();
    assert_eq!(rx_conf.rf.bb.sf, SpreadingFactor::_12);
    assert_eq!(rx_conf.rf.bb.bw, Bandwidth::_125KHz);
    // RX2
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    let rx_conf = radio.get_rxconfig().await.unwrap();
    assert_eq!(rx_conf.rf.frequency, 868525000);

    // Test for RX2 DR override
    assert_eq!(rx_conf.rf.bb.sf, SpreadingFactor::_10);
    assert_eq!(rx_conf.rf.bb.bw, Bandwidth::_125KHz);

    // RxComplete (no answer)
    assert!(*send_await_complete.lock().await);

    let (mut device, response) = task.await.unwrap();

    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.fhdr().data(), [5, 7]);
        }
        _ => panic!(),
    }

    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }

    // Trigger uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 4, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    // RX1
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    // RX2
    timer.fire_most_recent().await;
    radio.handle_timeout().await;
    // RxComplete (no answer)
    assert!(*send_await_complete.lock().await);

    let (mut device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::RxComplete) => (),
        _ => panic!(),
    }

    // Check that our uplink still contains required packets
    let mut uplink = radio.get_last_uplink().await;
    match uplink.get_payload() {
        PhyPayload::Data(DataPayload::Encrypted(data)) => {
            assert_eq!(data.fhdr().data(), [5, 7]);
        }
        _ => panic!(),
    }

    // Trigger uplink
    let complete = send_await_complete.clone();
    let task = tokio::spawn(async move {
        let response = device.send(&[1, 2, 3], 5, false).await;
        let mut complete = complete.lock().await;
        *complete = true;
        (device, response)
    });

    fn add_disabled_channel(_uplink: Option<Uplink>, _config: RfConfig, buf: &mut [u8]) -> usize {
        // NewChannelReq - add new channel to slot 3
        // LinkADRReq - channelmask in bank = 1, mask = 0b111 (effectively disabling new channel)
        build_mac(buf, "0703886684500350070001", 2)
    }
    timer.fire_most_recent().await;
    radio.handle_rxtx(add_disabled_channel).await;

    let (device, response) = task.await.unwrap();
    match response {
        Ok(SendResponse::DownlinkReceived(_)) => {}
        _ => panic!(),
    }

    let session = device.mac.get_session().unwrap();
    let data = session.uplink.mac_commands();
    // RxParamSetupAns has been dropped...
    assert_eq!(parse_uplink_mac_commands(data).count(), 2);
    assert_eq!(data, [7, 3, 3, 7]);
}
