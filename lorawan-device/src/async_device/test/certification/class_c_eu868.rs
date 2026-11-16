//! LoRaWAN 1.0.4 Class C certification testcases for EU868.
//!
use super::{net, util};
use crate::async_device::ListenResponse;
use net::{Net, Pkt};

use lorawan::parser::FrmPayload;

/// Based on TP_C_EU868_ED_MAC_104_BV_000 - Testing for downlinks on different RXC reception slots.
#[cfg(feature = "class-c")]
#[tokio::test]
async fn eu868_class_c_echo_payload_req_in_rxc() {
    let (radio, timer, mut device) =
        util::session_with_region(crate::region::EU868::new_eu868().into());
    device.enable_class_c();
    let mut net = Net::new(radio, timer, device).await;

    let r = net.listen(vec![Pkt::cp_cmd("08010203")]).await;

    assert!(matches!(r.response(), Ok(ListenResponse::UplinkPrepared)));
    assert_eq!(r.uplinks().len(), 1, "EchoIncPayloadReq must be answered");
    let mut uplink = r.uplink(0).clone();
    let dl = util::decrypt_uplink(&mut uplink);
    assert_eq!(dl.f_port(), Some(224));
    assert_eq!(dl.frm_payload(), FrmPayload::Data(&[0x08, 0x02, 0x03, 0x04]));
}
