//! LoRaWAN 1.0.4 Certification testcase: Join-Accept with CFList for
//! Dynamic Channel (DC) plan devices.
//!
//! The Join-Accept carries one additional channel (867.7 MHz) in a Type 0
//! CFList. The DUT must join and enable that channel together with the
//! default channels.
use super::util;
use crate::async_device::JoinResponse;
use crate::radio::RfConfig;
use crate::test_util::{
    Uplink, get_crypto, get_dev_addr, get_network_crypto, get_otaa_credentials,
};

use lorawan::creator::JoinAccept;
use lorawan::parser::{self, CfList, Frequency, JoinNonce, NetId, PhyPayload};
use lorawan::types::DLSettings;

fn handle_join_request_with_cflist(
    uplink: Option<Uplink>,
    _config: RfConfig,
    rx_buffer: &mut [u8],
) -> usize {
    let mut uplink = uplink.expect("expected a Join-Request uplink");
    let join_request = match parser::parse(uplink.data_mut()).unwrap() {
        PhyPayload::JoinRequest(join_request) => join_request,
        _ => panic!("expected a Join-Request"),
    };
    assert!(join_request.validate_mic(&get_crypto()));

    let accept = JoinAccept {
        join_nonce: JoinNonce::from_wire_bytes([1; 3]),
        net_id: NetId::from_wire_bytes([1; 3]),
        dev_addr: get_dev_addr(),
        dl_settings: DLSettings::new(0),
        rx_delay: 1,
        c_f_list: Some(CfList::DynamicChannel([
            Frequency::from_hz(867_700_000),
            Frequency::from_hz(0),
            Frequency::from_hz(0),
            Frequency::from_hz(0),
            Frequency::from_hz(0),
        ])),
    };
    accept.build_into(rx_buffer, &get_network_crypto()).unwrap().len()
}

#[tokio::test]
#[should_panic(expected = "the CFList channel must be enabled after the join")]
async fn eu868_join_accept_cflist_enables_channel() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut mask = device.mac.region.channel_mask_get();
    mask.set_channel(3, false);
    device.mac.region.channel_mask_set(mask);
    assert_eq!(device.mac.region.channel_mask_get().is_enabled(3), Ok(false));

    let task = tokio::spawn(async move {
        let response = device.join(&get_otaa_credentials()).await;
        (device, response)
    });

    timer.fire_most_recent().await; // join RX1 start
    radio.handle_rxtx(handle_join_request_with_cflist).await;

    let (device, response) = task.await.unwrap();
    assert!(matches!(response, Ok(JoinResponse::JoinSuccess)));
    assert_eq!(
        device.mac.region.channel_mask_get().is_enabled(3),
        Ok(true),
        "the CFList channel must be enabled after the join"
    );
}
