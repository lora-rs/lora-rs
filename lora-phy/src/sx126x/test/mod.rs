mod fixtures;
use fixtures::{get_sx126x, Delayer, TestFixture};

use crate::mod_traits::RadioKind;
use smtc_modem_cores::sx126x::*;

#[tokio::test]
async fn test_sleep_cold_start() {
    let smtc_sx126x = Context::new(TestFixture::new());
    smtc_sx126x.set_sleep(SleepCfg::ColdStart);
    let mut sx1261 = get_sx126x();
    sx1261.set_sleep(false, &mut Delayer).await.unwrap();
    assert_eq!(sx1261.take_spi(), smtc_sx126x.inner);
}

#[tokio::test]
async fn test_sleep_warm_start() {
    let smtc_sx126x = Context::new(TestFixture::new());
    smtc_sx126x.set_sleep(SleepCfg::WarmStart);
    let mut sx1261 = get_sx126x();
    sx1261.set_sleep(true, &mut Delayer).await.unwrap();
    assert_eq!(sx1261.take_spi(), smtc_sx126x.inner);
}
