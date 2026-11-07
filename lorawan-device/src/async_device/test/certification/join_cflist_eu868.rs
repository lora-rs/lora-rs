//! LoRaWAN 1.0.4 Certification testcase: Join-Accept with CFList for
//! Dynamic Channel (DC) plan devices.
//!
//! The Join-Accept carries one additional channel (867.7 MHz) in a Type 0
//! CFList. The DUT must join and enable that channel together with the
//! default channels.
use super::{net, util};
use net::{ExchangeAction, Net, Pkt};

use lorawan::parser::{CfList, Frequency};

#[tokio::test]
async fn eu868_join_accept_cflist_enables_channel() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    let mut mask = device.mac.region.channel_mask_get();
    mask.set_channel(3, false);
    device.mac.region.channel_mask_set(mask);
    assert_eq!(device.mac.region.channel_mask_get().is_enabled(3), Ok(false));
    let mut net = Net::new(radio, timer, device).await;

    let cf_list = CfList::DynamicChannel([
        Frequency::from_hz(867_700_000),
        Frequency::from_hz(0),
        Frequency::from_hz(0),
        Frequency::from_hz(0),
        Frequency::from_hz(0),
    ]);
    let accept = Pkt::join_accept().rx_delay(1).cf_list(cf_list);
    let r = net.join(ExchangeAction::rx1(accept)).await;
    r.expect_joined();
    util::assert_join_request(r.uplink(0));
    assert_eq!(
        net.device().mac.region.channel_mask_get().is_enabled(3),
        Ok(true),
        "the CFList channel must be enabled after the join"
    );
}
