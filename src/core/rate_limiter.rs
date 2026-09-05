use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<LimiterState>>,
}

#[derive(Debug)]
struct LimiterState {
    capacity: f64,
    tokens: f64,
    refill_rate: f64, // tokens per second
    last_update: Instant,
}

impl RateLimiter {
    pub fn new(requests: usize, seconds: u64, burst: usize) -> Self {
        let capacity = burst as f64;
        let refill_rate = (requests as f64) / (seconds.max(1) as f64);
        Self {
            state: Arc::new(Mutex::new(LimiterState {
                capacity,
                tokens: capacity,
                refill_rate,
                last_update: Instant::now(),
            })),
        }
    }

    pub async fn acquire(&self) {
        loop {
            let mut state = self.state.lock().await;
            let now = Instant::now();
            let elapsed = now.duration_since(state.last_update).as_secs_f64();
            state.last_update = now;
            state.tokens = (state.tokens + elapsed * state.refill_rate).min(state.capacity);

            if state.tokens >= 1.0 {
                state.tokens -= 1.0;
                return;
            }

            let wait_secs = (1.0 - state.tokens) / state.refill_rate;
            drop(state);
            tokio::time::sleep(Duration::from_secs_f64(wait_secs.max(0.005))).await;
        }
    }
}
