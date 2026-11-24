use crate::sx126x::{Config, Sx126x, Sx1261, Sx1262};
pub use crate::test_fixtures::{CommandStreamFixture, Delayer, DummyVariant, Ops};

pub fn get_sx126x() -> Sx126x<CommandStreamFixture, DummyVariant, Sx1261> {
    Sx126x::new(
        CommandStreamFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1261,
            tcxo_ctrl: None,
            use_dcdc: false,
            rx_boost: true,
        },
    )
}

pub fn get_sx1262() -> Sx126x<CommandStreamFixture, DummyVariant, Sx1262> {
    Sx126x::new(
        CommandStreamFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1262,
            tcxo_ctrl: None,
            use_dcdc: false,
            rx_boost: true,
        },
    )
}
