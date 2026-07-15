/// The possible IRQ set for mock radio
#[derive(Clone, Copy)]
pub enum IrqMask {
    /// Default
    None = 0x0000,
    /// When Tx is done
    TxDone = 0x0001,
    /// When Rx is done
    RxDone = 0x0002,
    /// For Rx check
    PreambleDetected = 0x0004,
    /// For Rx check
    SyncwordValid = 0x0008,
    /// For Rx check
    HeaderError = 0x0020,
    /// For Rx check
    CRCError = 0x0040,
    /// CAD is done
    CADDone = 0x0080,
    /// CAD detected activity
    CADActivityDetected = 0x0100,
    /// Timeout
    RxTxTimeout = 0x0200,
    /// All of the above flags
    All = 0xFFFF,
}

impl IrqMask {
    /// Get enum value
    pub fn value(self) -> u16 {
        self as u16
    }

    /// To check if a flag is set
    pub fn is_set(self, mask: u16) -> bool {
        self.value() & mask == self.value()
    }
}
