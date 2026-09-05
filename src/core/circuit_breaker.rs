use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half-open"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,
    pub recovery_duration: Duration,
    pub success_threshold: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_duration: Duration::from_secs(60),
            success_threshold: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStat {
    pub engine: String,
    pub state: String,
    pub failure_count: usize,
    pub last_changed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_response_ms: Option<u64>,
}

#[derive(Debug)]
pub struct CircuitBreaker {
    name: String,
    state: CircuitState,
    config: CircuitBreakerConfig,
    failure_count: usize,
    success_count: usize,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
    last_state_change_utc: DateTime<Utc>,
    success_latency_sum: Duration,
    success_samples: u64,
}

impl CircuitBreaker {
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            state: CircuitState::Closed,
            config,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            last_state_change: Instant::now(),
            last_state_change_utc: Utc::now(),
            success_latency_sum: Duration::ZERO,
            success_samples: 0,
        }
    }

    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= self.config.recovery_duration {
                        self.set_state(CircuitState::HalfOpen);
                        self.success_count = 0;
                        info!(
                            "Circuit breaker for engine '{}': recovery timeout elapsed, transitioning to half-open",
                            self.name
                        );
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&mut self, elapsed: Duration) {
        if elapsed > Duration::ZERO {
            self.success_latency_sum += elapsed;
            self.success_samples += 1;
        }

        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.set_state(CircuitState::Closed);
                    self.failure_count = 0;
                    self.success_count = 0;
                    info!(
                        "Circuit breaker for engine '{}': recovered, transitioning to closed",
                        self.name
                    );
                }
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            _ => {}
        }
    }

    pub fn record_failure(&mut self) {
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    self.set_state(CircuitState::Open);
                    warn!(
                        "Circuit breaker for engine '{}': failure threshold reached ({}), transitioning to open",
                        self.name, self.failure_count
                    );
                }
            }
            CircuitState::HalfOpen => {
                self.set_state(CircuitState::Open);
                self.success_count = 0;
                warn!(
                    "Circuit breaker for engine '{}': failure occurred during half-open, reopening circuit",
                    self.name
                );
            }
            _ => {}
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    pub fn stats(&self) -> CircuitBreakerStat {
        let retry_in = if self.state == CircuitState::Open {
            self.last_failure_time.map(|lf| {
                let elapsed = lf.elapsed();
                if elapsed < self.config.recovery_duration {
                    (self.config.recovery_duration - elapsed).as_secs()
                } else {
                    0
                }
            })
        } else {
            None
        };

        let avg_response_ms =
            (self.success_latency_sum.as_millis() as u64).checked_div(self.success_samples);

        CircuitBreakerStat {
            engine: self.name.clone(),
            state: self.state.to_string(),
            failure_count: self.failure_count,
            last_changed: self.last_state_change_utc.to_rfc3339(),
            retry_in,
            avg_response_ms,
        }
    }

    fn set_state(&mut self, state: CircuitState) {
        self.state = state;
        self.last_state_change = Instant::now();
        self.last_state_change_utc = Utc::now();
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerManager {
    breakers: Arc<Mutex<HashMap<String, CircuitBreaker>>>,
    config: CircuitBreakerConfig,
}

impl Default for CircuitBreakerManager {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
}

impl CircuitBreakerManager {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    pub async fn can_execute(&self, engine: &str) -> bool {
        let mut map = self.breakers.lock().await;
        let cb = map
            .entry(engine.to_string())
            .or_insert_with(|| CircuitBreaker::new(engine, self.config.clone()));
        cb.can_execute()
    }

    pub async fn record_success(&self, engine: &str, elapsed: Duration) {
        let mut map = self.breakers.lock().await;
        if let Some(cb) = map.get_mut(engine) {
            cb.record_success(elapsed);
        }
    }

    pub async fn record_failure(&self, engine: &str) {
        let mut map = self.breakers.lock().await;
        let cb = map
            .entry(engine.to_string())
            .or_insert_with(|| CircuitBreaker::new(engine, self.config.clone()));
        cb.record_failure();
    }

    pub async fn all_stats(&self) -> Vec<CircuitBreakerStat> {
        let map = self.breakers.lock().await;
        map.values().map(|cb| cb.stats()).collect()
    }

    pub async fn engine_state(&self, engine: &str) -> CircuitState {
        let mut map = self.breakers.lock().await;
        let cb = map
            .entry(engine.to_string())
            .or_insert_with(|| CircuitBreaker::new(engine, self.config.clone()));
        cb.state()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_state_transition() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_duration: Duration::from_millis(50),
            success_threshold: 1,
        };
        let mut cb = CircuitBreaker::new("test-engine", cfg);

        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.can_execute());

        // First failure
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.can_execute());

        // Second failure -> opens circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());

        // Sleep past recovery timeout
        std::thread::sleep(Duration::from_millis(60));
        assert!(cb.can_execute());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Success in half-open -> closes circuit
        cb.record_success(Duration::from_millis(10));
        assert_eq!(cb.state(), CircuitState::Closed);

        let stats = cb.stats();
        assert_eq!(stats.engine, "test-engine");
        assert_eq!(stats.state, "closed");
        assert_eq!(stats.avg_response_ms, Some(10));
    }
}
