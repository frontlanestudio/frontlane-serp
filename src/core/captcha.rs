use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// A non-Cloudflare CAPTCHA challenge detected on a page, and enough
/// information to submit it to a solver. Unlike Cloudflare's Turnstile (see
/// `CaptchaSolver::solve_cloudflare_challenge`), solving one of these
/// returns a raw response token rather than a reusable cookie: the token
/// has to be submitted back to whatever endpoint or form the page itself
/// expects, which varies site to site and isn't something this crate can
/// generalize -- see `crate::core::types::ExtractedContent::captcha_token`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptchaChallengeKind {
    RecaptchaV2 {
        site_key: String,
    },
    RecaptchaV3 {
        site_key: String,
        /// The `action` parameter the page's own `grecaptcha.execute()` call
        /// used, when it could be recovered from the page source. Both
        /// providers accept this being absent, but pass it through when we
        /// have it since some scoring depends on it matching.
        action: Option<String>,
    },
    HCaptcha {
        site_key: String,
    },
}

impl CaptchaChallengeKind {
    /// Short machine-readable label, used as `ExtractedContent::captcha_challenge`.
    pub fn kind_str(&self) -> &'static str {
        match self {
            CaptchaChallengeKind::RecaptchaV2 { .. } => "recaptcha_v2",
            CaptchaChallengeKind::RecaptchaV3 { .. } => "recaptcha_v3",
            CaptchaChallengeKind::HCaptcha { .. } => "hcaptcha",
        }
    }

    pub fn site_key(&self) -> &str {
        match self {
            CaptchaChallengeKind::RecaptchaV2 { site_key } => site_key,
            CaptchaChallengeKind::RecaptchaV3 { site_key, .. } => site_key,
            CaptchaChallengeKind::HCaptcha { site_key } => site_key,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CaptchaSolver {
    pub config: CaptchaSolverConfig,
    pub mock_solution: Option<CloudflareClearance>,
    /// Mock token for `solve_third_party_captcha`, mirroring `mock_solution`
    /// for Cloudflare -- lets tests exercise the reCAPTCHA/hCaptcha path
    /// without a live provider call.
    pub mock_third_party_token: Option<String>,
}

impl CaptchaSolver {
    pub fn new(config: CaptchaSolverConfig) -> Self {
        Self {
            config,
            mock_solution: None,
            mock_third_party_token: None,
        }
    }

    pub fn with_mock_solution(mut self, clearance: CloudflareClearance) -> Self {
        self.mock_solution = Some(clearance);
        self.config.enabled = true;
        self
    }

    pub fn with_mock_third_party_token(mut self, token: impl Into<String>) -> Self {
        self.mock_third_party_token = Some(token.into());
        self.config.enabled = true;
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
            && (self.mock_solution.is_some()
                || self.mock_third_party_token.is_some()
                || !self.config.api_key.trim().is_empty())
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
                self.solve_with_capsolver(url, &domain, proxy_url, user_agent)
                    .await
            }
            _ => {
                self.solve_with_2captcha(url, &domain, proxy_url, user_agent)
                    .await
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
            .timeout(std::time::Duration::from_secs(
                self.config.max_poll_timeout_secs,
            ))
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
                let desc = create_json
                    .get("errorDescription")
                    .and_then(|v| v.as_str())
                    .unwrap_or(err_code);
                return Err(SerpError::ChallengeSolver(format!(
                    "CapSolver error: {}",
                    desc
                )));
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

            let Ok(res) = result_res else {
                continue;
            };
            let Ok(res_json) = res.json::<serde_json::Value>().await else {
                continue;
            };

            let status = res_json
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if status == "ready" {
                let solution = res_json.get("solution").ok_or_else(|| {
                    record_solver_failure();
                    SerpError::ChallengeSolver(
                        "Missing solution object in CapSolver response".to_string(),
                    )
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
                return Err(SerpError::ChallengeSolver(
                    "CapSolver reported task failure".to_string(),
                ));
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
            .timeout(std::time::Duration::from_secs(
                self.config.max_poll_timeout_secs,
            ))
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
            let err = in_json
                .get("request")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN_ERROR");
            return Err(SerpError::ChallengeSolver(format!(
                "2Captcha submission error: {}",
                err
            )));
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

            let Ok(res) = client.get(&poll_url).send().await else {
                continue;
            };
            let Ok(res_json) = res.json::<serde_json::Value>().await else {
                continue;
            };

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
                    return Err(SerpError::ChallengeSolver(format!(
                        "2Captcha error: {}",
                        req_status
                    )));
                }
            }
        }

        record_solver_failure();
        Err(SerpError::ChallengeSolver(format!(
            "2captcha solver timed out solving challenge for {}",
            url
        )))
    }

    /// Solve a reCAPTCHA v2/v3 or hCaptcha challenge detected on a page (see
    /// `crate::extract::detect_third_party_captcha`), returning the raw
    /// response token. Unlike `solve_cloudflare_challenge`, there's no
    /// cookie to cache and replay -- the caller is responsible for
    /// submitting the token to whatever the target page expects.
    pub async fn solve_third_party_captcha(
        &self,
        url: &str,
        challenge: &CaptchaChallengeKind,
        proxy_url: Option<&str>,
    ) -> Result<String> {
        record_solver_attempt();

        if let Some(ref token) = self.mock_third_party_token {
            record_solver_success();
            return Ok(token.clone());
        }

        let api_key = self.config.api_key.trim();
        if api_key.is_empty() {
            record_solver_failure();
            return Err(SerpError::ChallengeSolver(
                "Captcha API key not configured for solver".to_string(),
            ));
        }

        let provider = self.config.provider.to_lowercase();
        match provider.as_str() {
            "capsolver" => {
                self.solve_third_party_with_capsolver(url, challenge, proxy_url)
                    .await
            }
            _ => {
                self.solve_third_party_with_2captcha(url, challenge, proxy_url)
                    .await
            }
        }
    }

    async fn solve_third_party_with_capsolver(
        &self,
        url: &str,
        challenge: &CaptchaChallengeKind,
        proxy_url: Option<&str>,
    ) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(
                self.config.max_poll_timeout_secs,
            ))
            .build()?;

        let has_proxy = proxy_url.map(|p| !p.trim().is_empty()).unwrap_or(false);

        let mut task_payload = match challenge {
            CaptchaChallengeKind::RecaptchaV2 { site_key } => serde_json::json!({
                "type": if has_proxy { "ReCaptchaV2Task" } else { "ReCaptchaV2TaskProxyLess" },
                "websiteURL": url,
                "websiteKey": site_key,
            }),
            CaptchaChallengeKind::RecaptchaV3 { site_key, action } => {
                let mut payload = serde_json::json!({
                    "type": if has_proxy { "ReCaptchaV3Task" } else { "ReCaptchaV3TaskProxyLess" },
                    "websiteURL": url,
                    "websiteKey": site_key,
                });
                if let Some(a) = action {
                    payload["pageAction"] = serde_json::json!(a);
                }
                payload
            }
            CaptchaChallengeKind::HCaptcha { site_key } => serde_json::json!({
                "type": if has_proxy { "HCaptchaTask" } else { "HCaptchaTaskProxyLess" },
                "websiteURL": url,
                "websiteKey": site_key,
            }),
        };

        if has_proxy {
            if let Some(p) = proxy_url {
                task_payload["proxy"] = serde_json::json!(p);
            }
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
                let desc = create_json
                    .get("errorDescription")
                    .and_then(|v| v.as_str())
                    .unwrap_or(err_code);
                return Err(SerpError::ChallengeSolver(format!(
                    "CapSolver error: {}",
                    desc
                )));
            }
        }

        let task_id = create_json
            .get("taskId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                record_solver_failure();
                SerpError::ChallengeSolver("No taskId returned by CapSolver".to_string())
            })?;

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

            let Ok(res) = result_res else {
                continue;
            };
            let Ok(res_json) = res.json::<serde_json::Value>().await else {
                continue;
            };

            let status = res_json
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if status == "ready" {
                let solution = res_json.get("solution").ok_or_else(|| {
                    record_solver_failure();
                    SerpError::ChallengeSolver(
                        "Missing solution object in CapSolver response".to_string(),
                    )
                })?;

                // The documented field is `gRecaptchaResponse` for all three
                // task families, but fall back defensively to a couple of
                // alternates seen in practice rather than hard-failing on a
                // naming mismatch we can't verify without a live account.
                let token = solution
                    .get("gRecaptchaResponse")
                    .or_else(|| solution.get("token"))
                    .or_else(|| solution.get("captchaKey"))
                    .and_then(|v| v.as_str());

                if let Some(t) = token {
                    if !t.is_empty() {
                        record_solver_success();
                        return Ok(t.to_string());
                    }
                }
            } else if status == "failed" {
                record_solver_failure();
                return Err(SerpError::ChallengeSolver(
                    "CapSolver reported task failure".to_string(),
                ));
            }
        }

        record_solver_failure();
        Err(SerpError::ChallengeSolver(format!(
            "CapSolver timed out solving challenge for {}",
            url
        )))
    }

    async fn solve_third_party_with_2captcha(
        &self,
        url: &str,
        challenge: &CaptchaChallengeKind,
        proxy_url: Option<&str>,
    ) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(
                self.config.max_poll_timeout_secs,
            ))
            .build()?;

        let mut form_params: Vec<(&str, String)> = vec![
            ("key", self.config.api_key.clone()),
            ("pageurl", url.to_string()),
            ("json", "1".to_string()),
        ];

        match challenge {
            CaptchaChallengeKind::RecaptchaV2 { site_key } => {
                form_params.push(("method", "userrecaptcha".to_string()));
                form_params.push(("googlekey", site_key.clone()));
            }
            CaptchaChallengeKind::RecaptchaV3 { site_key, action } => {
                form_params.push(("method", "userrecaptcha".to_string()));
                form_params.push(("version", "v3".to_string()));
                form_params.push(("googlekey", site_key.clone()));
                form_params.push((
                    "action",
                    action.clone().unwrap_or_else(|| "verify".to_string()),
                ));
                form_params.push(("min_score", "0.3".to_string()));
            }
            CaptchaChallengeKind::HCaptcha { site_key } => {
                form_params.push(("method", "hcaptcha".to_string()));
                form_params.push(("sitekey", site_key.clone()));
            }
        }

        if let Some(p) = proxy_url {
            if !p.trim().is_empty() {
                form_params.push(("proxy", p.to_string()));
                form_params.push(("proxytype", "HTTP".to_string()));
            }
        }

        let form_refs: Vec<(&str, &str)> =
            form_params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let in_res = client
            .post("https://2captcha.com/in.php")
            .form(&form_refs)
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
            let err = in_json
                .get("request")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN_ERROR");
            return Err(SerpError::ChallengeSolver(format!(
                "2Captcha submission error: {}",
                err
            )));
        }

        let request_id = in_json
            .get("request")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                record_solver_failure();
                SerpError::ChallengeSolver("No request ID from 2Captcha".to_string())
            })?;

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(self.config.max_poll_timeout_secs);
        let poll_interval = std::time::Duration::from_millis(self.config.poll_interval_ms);

        while start.elapsed() < timeout {
            tokio::time::sleep(poll_interval).await;

            let poll_url = format!(
                "https://2captcha.com/res.php?key={}&action=get&id={}&json=1",
                self.config.api_key, request_id
            );

            let Ok(res) = client.get(&poll_url).send().await else {
                continue;
            };
            let Ok(res_json) = res.json::<serde_json::Value>().await else {
                continue;
            };

            if res_json.get("status").and_then(|v| v.as_i64()) == Some(1) {
                if let Some(token) = res_json.get("request").and_then(|v| v.as_str()) {
                    record_solver_success();
                    return Ok(token.to_string());
                }
            } else if let Some(req_status) = res_json.get("request").and_then(|v| v.as_str()) {
                if req_status != "CAPCHA_NOT_READY" {
                    record_solver_failure();
                    return Err(SerpError::ChallengeSolver(format!(
                        "2Captcha error: {}",
                        req_status
                    )));
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

    #[test]
    fn test_challenge_kind_str_and_site_key() {
        let v2 = CaptchaChallengeKind::RecaptchaV2 {
            site_key: "6Le-key".to_string(),
        };
        assert_eq!(v2.kind_str(), "recaptcha_v2");
        assert_eq!(v2.site_key(), "6Le-key");

        let v3 = CaptchaChallengeKind::RecaptchaV3 {
            site_key: "6Le-key-v3".to_string(),
            action: Some("login".to_string()),
        };
        assert_eq!(v3.kind_str(), "recaptcha_v3");
        assert_eq!(v3.site_key(), "6Le-key-v3");

        let hc = CaptchaChallengeKind::HCaptcha {
            site_key: "hc-key".to_string(),
        };
        assert_eq!(hc.kind_str(), "hcaptcha");
        assert_eq!(hc.site_key(), "hc-key");
    }

    #[tokio::test]
    async fn test_solve_third_party_captcha_uses_mock() {
        let solver = CaptchaSolver::new(CaptchaSolverConfig::default())
            .with_mock_third_party_token("mock-token-123");
        assert!(solver.is_enabled());

        let challenge = CaptchaChallengeKind::RecaptchaV2 {
            site_key: "any-site-key".to_string(),
        };
        let token = solver
            .solve_third_party_captcha("https://example.com/gated", &challenge, None)
            .await
            .unwrap();
        assert_eq!(token, "mock-token-123");
    }

    #[tokio::test]
    async fn test_solve_third_party_captcha_no_api_key_fails() {
        let solver = CaptchaSolver::new(CaptchaSolverConfig::default());
        let challenge = CaptchaChallengeKind::HCaptcha {
            site_key: "any-site-key".to_string(),
        };
        let result = solver
            .solve_third_party_captcha("https://example.com/gated", &challenge, None)
            .await;
        assert!(result.is_err());
    }
}
