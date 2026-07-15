#[derive(Clone, Copy)]
pub enum IrqMask {
    None = 0x0000,
    TxDone = 0x0001,
    RxDone = 0x0002,
    PreambleDetected = 0x0004,
    SyncwordValid = 0x0008,
    HeaderValid = 0x0010,
    HeaderError = 0x0020,
    CRCError = 0x0040,
    CADDone = 0x0080,
    CADActivityDetected = 0x0100,
    RxTxTimeout = 0x0200,
    All = 0xFFFF,
}

impl IrqMask {
    pub fn value(self) -> u16 {
        self as u16
    }

    pub fn is_set(self, mask: u16) -> bool {
        self.value() & mask == self.value()
    }
}
