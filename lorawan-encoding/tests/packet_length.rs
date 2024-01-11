use lorawan::packet_length;

#[test]
fn test_mac_join_max() {
    assert_eq!(33, packet_length::phy::mac::JOIN_ACCEPT_MAX);
}
