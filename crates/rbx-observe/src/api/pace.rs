//! Request spacing.
//!
//! Roblox enforces a **sliding-window** quota per host and per IP, not a flat
//! rate: a long run trips 429 even at a steady interval, because the window
//! fills up regardless of how evenly requests arrive. Measured against
//! `games.roblox.com` with real universe ids: a short burst survives at 1
//! req/s, 429 appears around 400ms, and a sustained run at a fixed 1.5s still
//! lost ~8% of its batches.
//!
//! So the pacer widens itself on every 429 and never narrows within a process.
//! It converges on whatever the window actually allows instead of guessing.

use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};

pub struct Pacer {
    state: Mutex<State>,
    max: Duration,
}

struct State {
    interval: Duration,
    last: Option<Instant>,
}

impl Pacer {
    pub fn new(interval: Duration, max: Duration) -> Self {
        Self {
            state: Mutex::new(State {
                interval,
                last: None,
            }),
            max,
        }
    }

    /// No spacing at all. Tests would otherwise pay the real interval per
    /// request against a local mock that has no quota.
    #[cfg(test)]
    pub fn disabled() -> Self {
        Self::new(Duration::ZERO, Duration::ZERO)
    }

    /// Sleeps until at least `interval` has elapsed since the previous request
    /// **started**. Holding the lock across the sleep is what serialises
    /// callers: two concurrent requests queue rather than both deciding they
    /// are allowed to go now.
    pub async fn wait(&self) {
        let mut state = self.state.lock().await;
        if let Some(last) = state.last {
            let elapsed = last.elapsed();
            if elapsed < state.interval {
                sleep(state.interval - elapsed).await;
            }
        }
        state.last = Some(Instant::now());
    }

    /// Called on every throttled response, including one a retry later
    /// rescues. Waiting for a request to be lost entirely before reacting
    /// means reacting one full batch too late.
    pub async fn widen(&self) {
        let mut state = self.state.lock().await;
        let widened = state.interval.mul_f32(1.5);
        state.interval = widened.min(self.max);
    }

    #[cfg(test)]
    pub async fn interval(&self) -> Duration {
        self.state.lock().await.interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn widening_stops_at_the_ceiling() {
        let pacer = Pacer::new(Duration::from_millis(1000), Duration::from_millis(2000));
        pacer.widen().await;
        assert_eq!(pacer.interval().await, Duration::from_millis(1500));
        pacer.widen().await;
        pacer.widen().await;
        assert_eq!(pacer.interval().await, Duration::from_millis(2000));
    }

    #[tokio::test]
    async fn a_disabled_pacer_never_sleeps() {
        let pacer = Pacer::disabled();
        let started = Instant::now();
        for _ in 0..50 {
            pacer.wait().await;
        }
        assert!(started.elapsed() < Duration::from_millis(200));
    }
}
