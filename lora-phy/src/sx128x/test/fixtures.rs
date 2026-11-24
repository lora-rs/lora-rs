use crate::sx128x::{Config, Sx128x, Sx1280, Sx1281};
pub use crate::test_fixtures::{CommandStreamFixture, Delayer, DummyVariant, Ops};

pub fn get_sx1280() -> Sx128x<CommandStreamFixture, DummyVariant, Sx1280> {
    Sx128x::new(
        CommandStreamFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1280,
            tcxo_used: false,
            use_dcdc: false,
        },
    )
}

pub fn get_sx1281() -> Sx128x<CommandStreamFixture, DummyVariant, Sx1281> {
    Sx128x::new(
        CommandStreamFixture::new(),
        DummyVariant,
        Config {
            chip: Sx1281,
            tcxo_used: false,
            use_dcdc: false,
        },
    )
}
