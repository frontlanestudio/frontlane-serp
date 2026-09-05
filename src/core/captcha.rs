use std::sync::atomic::{AtomicU64, Ordering};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::core::error::{Result, SerpError};

static SOLVER_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static SOLVER_SUCCESSES: AtomicU64 = AtomicU64::new(0);
static SOLVER_FAILURES: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaptchaSolverMetrics {
    pub solver_attempts: u64,
    pub solver_successes: u64,
    pub solver_failures: u64,
}

pub fn captcha_solver_metrics() -> CaptchaSolverMetrics {
    CaptchaSolverMetrics {
        solver_attempts: SOLVER_ATTEMPTS.load(Ordering::Relaxed),
        solver_successes: SOLVER_SUCCESSES.load(Ordering::Relaxed),
        solver_failures: SOLVER_FAILURES.load(Ordering::Relaxed),
    }
}

pub fn record_solver_attempt() {
    SOLVER_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_solver_success() {
    SOLVER_SUCCESSES.fetch_add(1, Ordering::Relaxed);
}

pub fn record_solver_failure() {
    SOLVER_FAILURES.fetch_add(1, Ordering::Relaxed);
}

pub fn reset_solver_metrics() {
    SOLVER_ATTEMPTS.store(0, Ordering::Relaxed);
    SOLVER_SUCCESSES.store(0, Ordering::Relaxed);
    SOLVER_FAILURES.store(0, Ordering::Relaxed);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareClearance {
    pub domain: String,
    pub cf_clearance: String,
    pub user_agent: String,
    pub expires_at: DateTime<Utc>,
    pub proxy_url: Option<String>,
}

impl CloudflareClearance {
    pub fn new(
        domain: impl Into<String>,
        cf_clearance: impl Into<String>,
        user_agent: impl Into<String>,
        ttl_secs: i64,
        proxy_url: Option<String>,
    ) -> Self {
        Self {
            domain: domain.into(),
            cf_clearance: cf_clearance.into(),
            user_agent: user_agent.into(),
            expires_at: Utc::now() + Duration::seconds(ttl_secs.max(60)),
            proxy_url,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaSolverConfig {
    pub enabled: bool,
    pub provider: String,
    pub api_key: String,
    pub poll_interval_ms: u64,
    pub max_poll_timeout_secs: u64,
}

impl Default for CaptchaSolverConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "2captcha".to_string(),
            api_key: String::new(),
            poll_interval_ms: 2000,
            max_poll_timeout_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CaptchaSolver {
    pub config: CaptchaSolverConfig,
    pub mock_solution: Option<CloudflareClearance>,
}

impl CaptchaSolver {
    pub fn new(config: CaptchaSolverConfig) -> Self {
        Self {
            config,
            mock_solution: None,
        }
    }

    pub fn with_mock_solution(mut self, clearance: CloudflareClearance) -> Self {
        self.mock_solution = Some(clearance);
        self.config.enabled = true;
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled && (self.mock_solution.is_some() || !self.config.api_key.trim().is_empty())
    }

    pub async fn solve_cloudflare_challenge(
        &self,
        url: &str,
        proxy_url: Option<&str>,
        user_agent: &str,
    ) -> Result<CloudflareClearance> {
        record_solver_attempt();

        if let Some(ref mock) = self.mock_solution {
            record_solver_success();
            return Ok(mock.clone());
        }

        let api_key = self.config.api_key.trim();
        if api_key.is_empty() {
            record_solver_failure();
            return Err(SerpError::ChallengeSolver(
                "Captcha API key not configured for solver".to_string(),
            ));
        }

        // Domain extraction
        let parsed_url = url::Url::parse(url)?;
        let domain = parsed_url.host_str().unwrap_or("").to_string();

        let provider = self.config.provider.to_lowercase();
        match provider.as_str() {
            "capsolver" => {
                self.solve_with_capsolver(url, &domain, proxy_url, user_agent).await
            }
            _ => {
                self.solve_with_2captcha(url, &domain, proxy_url, user_agent).await
            }
        }
    }

    async fn solve_with_capsolver(
        &self,
        url: &str,
        domain: &str,
        proxy_url: Option<&str>,
        user_agent: &str,
    ) -> Result<CloudflareClearance> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.config.max_poll_timeout_secs))
            .build()?;

        // 1. Create Task: AntiCloudflareTask or AntiTurnstileTask
        let mut task_payload = serde_json::json!({
            "type": "AntiCloudflareTask",
            "websiteURL": url,
            "metadata": {
                "action": "managed"
            }
        });

        if let Some(p) = proxy_url {
            task_payload["proxy"] = serde_json::json!(p);
        }

        let create_task_res = client
            .post("https://api.capsolver.com/createTask")
            .json(&serde_json::json!({
                "clientKey": self.config.api_key,
                "task": task_payload
            }))
            .send()
            .await
            .map_err(|e| {
                record_solver_failure();
                SerpError::ChallengeSolver(format!("CapSolver network error: {}", e))
            })?;

        let create_json: serde_json::Value = create_task_res.json().await.map_err(|e| {
            record_solver_failure();
            SerpError::ChallengeSolver(format!("Failed to parse CapSolver task response: {}", e))
        })?;

        if let Some(err_code) = create_json.get("errorCode").and_then(|v| v.as_str()) {
            if !err_code.is_empty() && err_code != "0" {
                record_solver_failure();
                let desc = create_json.get("errorDescription").and_then(|v| v.as_str()).unwrap_or(err_code);
                return Err(SerpError::ChallengeSolver(format!("CapSolver error: {}", desc)));
            }
        }

        let task_id = create_json
            .get("taskId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                record_solver_failure();
                SerpError::ChallengeSolver("No taskId returned by CapSolver".to_string())
            })?;

        // 2. Poll for solution
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(self.config.max_poll_timeout_secs);
        let poll_interval = std::time::Duration::from_millis(self.config.poll_interval_ms);

        while start.elapsed() < timeout {
            tokio::time::sleep(poll_interval).await;

            let result_res = client
                .post("https://api.capsolver.com/getTaskResult")
                .json(&serde_json::json!({
                    "clientKey": self.config.api_key,
                    "taskId": task_id
                }))
                .send()
                .await;

            let Ok(res) = result_res else { continue; };
            let Ok(res_json) = res.json::<serde_json::Value>().await else { continue; };

            let status = res_json.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status == "ready" {
                let solution = res_json.get("solution").ok_or_else(|| {
                    record_solver_failure();
                    SerpError::ChallengeSolver("Missing solution object in CapSolver response".to_string())
                })?;

                // CapSolver returns clearance cookie in solution.cookies.cf_clearance or solution.token
                let cf_clearance = solution
                    .get("cookies")
                    .and_then(|c| c.get("cf_clearance"))
                    .and_then(|v| v.as_str())
                    .or_else(|| solution.get("token").and_then(|v| v.as_str()))
                    .unwrap_or("");

                if !cf_clearance.is_empty() {
                    record_solver_success();
                    return Ok(CloudflareClearance::new(
                        domain,
                        cf_clearance,
                        user_agent,
                        1800, // 30 minutes clearance
                        proxy_url.map(String::from),
                    ));
                }
            } else if status == "failed" {
                record_solver_failure();
                return Err(SerpError::ChallengeSolver("CapSolver reported task failure".to_string()));
            }
        }

        record_solver_failure();
        Err(SerpError::ChallengeSolver(format!(
            "CapSolver timed out solving challenge for {}",
            url
        )))
    }

    async fn solve_with_2captcha(
        &self,
        url: &str,
        domain: &str,
        proxy_url: Option<&str>,
        user_agent: &str,
    ) -> Result<CloudflareClearance> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.config.max_poll_timeout_secs))
            .build()?;

        // 2Captcha in.php
        let mut form_params: Vec<(&str, &str)> = vec![
            ("key", &self.config.api_key),
            ("method", "turnstile"),
            ("pageurl", url),
            ("json", "1"),
        ];

        let proxy_str;
        if let Some(p) = proxy_url {
            proxy_str = p;
            form_params.push(("proxy", proxy_str));
            form_params.push(("proxytype", "HTTP"));
        }

        let in_res = client
            .post("https://2captcha.com/in.php")
            .form(&form_params)
            .send()
            .await
            .map_err(|e| {
                record_solver_failure();
                SerpError::ChallengeSolver(format!("2Captcha network error: {}", e))
            })?;

        let in_json: serde_json::Value = in_res.json().await.map_err(|e| {
            record_solver_failure();
            SerpError::ChallengeSolver(format!("Failed to parse 2Captcha response: {}", e))
        })?;

        if in_json.get("status").and_then(|v| v.as_i64()) != Some(1) {
            record_solver_failure();
            let err = in_json.get("request").and_then(|v| v.as_str()).unwrap_or("UNKNOWN_ERROR");
            return Err(SerpError::ChallengeSolver(format!("2Captcha submission error: {}", err)));
        }

        let request_id = in_json
            .get("request")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                record_solver_failure();
                SerpError::ChallengeSolver("No request ID from 2Captcha".to_string())
            })?;

        // Poll res.php
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(self.config.max_poll_timeout_secs);
        let poll_interval = std::time::Duration::from_millis(self.config.poll_interval_ms);

        while start.elapsed() < timeout {
            tokio::time::sleep(poll_interval).await;

            let poll_url = format!(
                "https://2captcha.com/res.php?key={}&action=get&id={}&json=1",
                self.config.api_key, request_id
            );

            let Ok(res) = client.get(&poll_url).send().await else { continue; };
            let Ok(res_json) = res.json::<serde_json::Value>().await else { continue; };

            if res_json.get("status").and_then(|v| v.as_i64()) == Some(1) {
                if let Some(token) = res_json.get("request").and_then(|v| v.as_str()) {
                    record_solver_success();
                    return Ok(CloudflareClearance::new(
                        domain,
                        token,
                        user_agent,
                        1800,
                        proxy_url.map(String::from),
                    ));
                }
            } else if let Some(req_status) = res_json.get("request").and_then(|v| v.as_str()) {
                if req_status != "CAPCHA_NOT_READY" {
                    record_solver_failure();
                    return Err(SerpError::ChallengeSolver(format!("2Captcha error: {}", req_status)));
                }
            }
        }

        record_solver_failure();
        Err(SerpError::ChallengeSolver(format!(
            "2captcha solver timed out solving challenge for {}",
            url
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_captcha_metrics() {
        reset_solver_metrics();
        let m = captcha_solver_metrics();
        assert_eq!(m.solver_attempts, 0);

        record_solver_attempt();
        record_solver_success();
        let m2 = captcha_solver_metrics();
        assert_eq!(m2.solver_attempts, 1);
        assert_eq!(m2.solver_successes, 1);
        assert_eq!(m2.solver_failures, 0);
    }

    #[test]
    fn test_cloudflare_clearance_expiration() {
        let clearance = CloudflareClearance::new("example.com", "cookie123", "UA", 100, None);
        assert!(!clearance.is_expired());
    }
}
