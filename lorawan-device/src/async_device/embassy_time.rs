use embassy_time::{Duration, Instant};

use super::radio::Timer;

/// A [`Timer`] implementation based on [`embassy-time`].
pub struct EmbassyTimer {
    start: Instant,
}

impl EmbassyTimer {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }
}

impl Default for EmbassyTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer for EmbassyTimer {
    fn reset(&mut self) {
        self.start = Instant::now();
    }

    async fn at(&mut self, millis: u64) {
        embassy_time::Timer::at(self.start + Duration::from_millis(millis)).await
    }

    async fn delay_ms(&mut self, millis: u64) {
        embassy_time::Timer::after_millis(millis).await
    }
}
