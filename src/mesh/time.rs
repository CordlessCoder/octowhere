//! The board's local clock: embassy's time driver, in microseconds since boot.

use embassy_time::{Instant, Timer};

use octowhere_node::Time;

pub fn local() -> i64 {
    Instant::now().as_micros() as i64
}

pub fn until(local: i64) -> Timer {
    Timer::at(Instant::from_micros(local.max(0) as u64))
}

pub struct BoardTime;

impl Time for BoardTime {
    fn now(&self) -> i64 {
        local()
    }

    async fn until(&self, at: i64) {
        until(at).await;
    }
}
