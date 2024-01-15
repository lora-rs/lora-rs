pub mod phy {
    pub const MHDR_LEN: usize = 1;
    pub const MIC_LEN: usize = 4;

    pub mod join {
        use super::{MHDR_LEN, MIC_LEN};

        pub const JOIN_NONCE_LEN: usize = 3;
        pub const NET_ID_LEN: usize = 3;
        pub const DEV_ADDR_LEN: usize = 4;
        pub const DL_SETTINGS_LEN: usize = 1;
        pub const RX_DELAY_LEN: usize = 1;
        pub const CF_LIST_LEN: usize = 16;

        pub const JOIN_ACCEPT_PAYLOAD_LEN: usize =
            JOIN_NONCE_LEN + NET_ID_LEN + DEV_ADDR_LEN + DL_SETTINGS_LEN + RX_DELAY_LEN;
        pub const JOIN_ACCEPT_PAYLOAD_WITH_CFLIST_LEN: usize =
            JOIN_ACCEPT_PAYLOAD_LEN + CF_LIST_LEN;

        pub const JOIN_ACCEPT_LEN: usize = MHDR_LEN + JOIN_ACCEPT_PAYLOAD_LEN + MIC_LEN;
        pub const JOIN_ACCEPT_WITH_CFLIST_LEN: usize =
            MHDR_LEN + JOIN_ACCEPT_PAYLOAD_WITH_CFLIST_LEN + MIC_LEN;

        pub const JOIN_EUI_LEN: usize = 8;
        pub const DEV_EUI_LEN: usize = 8;
        pub const DEV_NONCE_LEN: usize = 2;
        pub const JOIN_REQUEST_PAYLOAD_LEN: usize = JOIN_EUI_LEN + DEV_EUI_LEN + DEV_NONCE_LEN;
        pub const JOIN_REQUEST_LEN: usize = MHDR_LEN + JOIN_REQUEST_PAYLOAD_LEN + MIC_LEN;
    }

    pub const PHY_PAYLOAD_MIN_LEN: usize = MHDR_LEN + mac::MAC_PAYLOAD_MIN + MIC_LEN;
    pub mod mac {
        pub const FPORT_LEN: usize = 1;
        pub mod fhdr {
            pub const DEV_ADDR_LEN: usize = 4;
            pub const FCTRL_LEN: usize = 1;
            pub const FCNT_LEN: usize = 2;
            pub const FOPTS_MIN_LEN: usize = 0;
            pub const FOPTS_MAX_LEN: usize = 15;

            pub const FHDR_MIN_LEN: usize = DEV_ADDR_LEN + FCTRL_LEN + FCNT_LEN + FOPTS_MIN_LEN;
            pub const FHDR_MAX_LEN: usize = DEV_ADDR_LEN + FCTRL_LEN + FCNT_LEN + FOPTS_MAX_LEN;
        }
        pub const MAC_PAYLOAD_MIN: usize = fhdr::FHDR_MIN_LEN;
    }
}
