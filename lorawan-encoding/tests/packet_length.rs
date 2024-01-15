use lorawan::packet_length;

#[test]
fn test_phy_payload_mac_min() {
    assert_eq!(12, packet_length::phy::PHY_PAYLOAD_MIN_LEN);
}
#[test]
fn test_mac_join_accept_with_cflist() {
    assert_eq!(33, packet_length::phy::join::JOIN_ACCEPT_WITH_CFLIST_LEN);
}

#[test]
fn test_mac_join_accept() {
    assert_eq!(17, packet_length::phy::join::JOIN_ACCEPT_LEN);
}

#[test]
fn test_mac_join_request() {
    assert_eq!(23, packet_length::phy::join::JOIN_REQUEST_LEN);
}
