use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::warn;

use crate::core::engine::SearchEngine;
use crate::core::error::{Result, SerpError};
use crate::core::types::{Query, SearchResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
struct CircuitBreaker {
    state: CircuitState,
    consecutive_failures: usize,
    consecutive_successes: usize,
    failure_threshold: usize,
    success_threshold: usize,
    recovery_duration: Duration,
    last_state_change: Instant,
}

impl CircuitBreaker {
    fn new(failure_threshold: usize, recovery_seconds: u64, success_threshold: usize) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            failure_threshold: failure_threshold.max(1),
            success_threshold: success_threshold.max(1),
            recovery_duration: Duration::from_secs(recovery_seconds.max(1)),
            last_state_change: Instant::now(),
        }
    }

    fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if self.last_state_change.elapsed() >= self.recovery_duration {
                    self.state = CircuitState::HalfOpen;
                    self.consecutive_successes = 0;
                    self.last_state_change = Instant::now();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    fn record_result(&mut self, success: bool) {
        if success {
            match self.state {
                CircuitState::HalfOpen => {
                    self.consecutive_successes += 1;
                    if self.consecutive_successes >= self.success_threshold {
                        self.state = CircuitState::Closed;
                        self.consecutive_failures = 0;
                        self.last_state_change = Instant::now();
                    }
                }
                CircuitState::Closed => {
                    self.consecutive_failures = 0;
                }
                _ => {}
            }
        } else {
            match self.state {
                CircuitState::Closed => {
                    self.consecutive_failures += 1;
                    if self.consecutive_failures >= self.failure_threshold {
                        self.state = CircuitState::Open;
                        self.last_state_change = Instant::now();
                    }
                }
                CircuitState::HalfOpen => {
                    self.state = CircuitState::Open;
                    self.last_state_change = Instant::now();
                }
                _ => {}
            }
        }
    }
}

#[derive(Clone)]
pub struct ResilientSearcher {
    circuit_breakers: Arc<Mutex<HashMap<String, CircuitBreaker>>>,
    max_retries: usize,
    base_backoff: Duration,
}

impl ResilientSearcher {
    pub fn new(max_retries: usize) -> Self {
        Self {
            circuit_breakers: Arc::new(Mutex::new(HashMap::new())),
            max_retries,
            base_backoff: Duration::from_millis(200),
        }
    }

    pub async fn execute_search<E: SearchEngine + ?Sized>(
        &self,
        engine: &E,
        query: &Query,
    ) -> Result<Vec<SearchResult>> {
        let engine_name = engine.name().to_string();

        {
            let mut cbs = self.circuit_breakers.lock().await;
            let cb = cbs
                .entry(engine_name.clone())
                .or_insert_with(|| CircuitBreaker::new(5, 60, 2));
            if !cb.can_execute() {
                return Err(SerpError::CircuitBreakerOpen(engine_name));
            }
        }

        let mut attempts = 0;
        let max_attempts = self.max_retries + 1;
        let mut last_err = SerpError::Other("unknown error".to_string());

        while attempts < max_attempts {
            attempts += 1;
            match engine.search(query).await {
                Ok(results) => {
                    let mut cbs = self.circuit_breakers.lock().await;
                    if let Some(cb) = cbs.get_mut(&engine_name) {
                        cb.record_result(true);
                    }
                    return Ok(results);
                }
                Err(err) => {
                    warn!(
                        "Engine {} attempt {} failed: {}",
                        engine_name, attempts, err
                    );

                    let retryable = err.is_retryable();
                    last_err = err;

                    if !retryable || attempts >= max_attempts {
                        break;
                    }

                    let sleep_duration = self.base_backoff * (1 << (attempts - 1));
                    tokio::time::sleep(sleep_duration).await;
                }
            }
        }

        let mut cbs = self.circuit_breakers.lock().await;
        if let Some(cb) = cbs.get_mut(&engine_name) {
            cb.record_result(false);
        }

        Err(last_err)
    }

    pub async fn execute_image_search<E: SearchEngine + ?Sized>(
        &self,
        engine: &E,
        query: &Query,
    ) -> Result<Vec<SearchResult>> {
        let _engine_name = engine.name().to_string();
        engine.search_image(query).await
    }
}
