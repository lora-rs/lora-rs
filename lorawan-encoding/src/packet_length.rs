pub mod phy {
    pub const UPLINK_CRC: u8 = 2;

    pub const UPLINK_OVERHEAD: u8 = UPLINK_CRC;
    pub const DOWNLINK_OVERHEAD: u8 = 0;

    pub mod mac {
        pub const UPLINK_OVERHEAD: u8 = super::UPLINK_OVERHEAD + MHDR + MIC;
        pub const DOWNLINK_OVERHEAD: u8 = super::DOWNLINK_OVERHEAD + MHDR + MIC;

        pub const MHDR: u8 = 1;
        pub const MIC: u8 = 4;

        pub const JOIN_ACCEPT_MIN: u8 = DOWNLINK_OVERHEAD + 12;
        pub const JOIN_ACCEPT_MAX: u8 = JOIN_ACCEPT_MIN + 16;

        pub mod frm {
            pub mod fhdr {
                pub const DEV_ADDR: u8 = 4;
                pub const FCTRL: u8 = 1;
                pub const FCNT: u8 = 2;
                pub const FOPTS_MIN: u8 = 0;
                pub const FOPTS_MAX: u8 = 15;

                pub const MIN: u8 = DEV_ADDR + FCTRL + FCNT + FOPTS_MIN;
                pub const MAX: u8 = DEV_ADDR + FCTRL + FCNT + FOPTS_MAX;
            }
        }
    }
}
