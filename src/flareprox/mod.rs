pub mod client;
pub mod worker_script;

pub use client::{CloudflareClient, FlareProxDeployment, FlareProxError, FlareProxTestResult};
pub use worker_script::FLAREPROX_WORKER_JS;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlareProxConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub api_token: Option<String>,

    #[serde(default)]
    pub account_id: Option<String>,

    #[serde(default = "default_prefix")]
    pub worker_prefix: String,

    #[serde(default)]
    pub workers: Vec<String>,
}

fn default_prefix() -> String {
    "flareprox".to_string()
}

impl Default for FlareProxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_token: None,
            account_id: None,
            worker_prefix: default_prefix(),
            workers: Vec::new(),
        }
    }
}

/// Tries to automatically detect the Cloudflare OAuth token from local Wrangler config
pub fn detect_wrangler_token() -> Option<String> {
    let mut candidate_paths = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        candidate_paths.push(format!(
            "{}/Library/Preferences/.wrangler/config/default.toml",
            home
        ));
        candidate_paths.push(format!("{}/.wrangler/config/default.toml", home));
        candidate_paths.push(format!("{}/.config/.wrangler/config/default.toml", home));
    }
    for path_str in candidate_paths {
        let p = std::path::Path::new(&path_str);
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(p) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("oauth_token") {
                        if let Some(val) = trimmed.split('=').nth(1) {
                            let clean = val.trim().trim_matches('"').trim_matches('\'').trim();
                            if !clean.is_empty() {
                                return Some(clean.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

impl FlareProxConfig {
    /// Resolves credentials with fallback to standard environment variables or wrangler config
    pub fn resolved_credentials(&self) -> (Option<String>, Option<String>) {
        let token = self
            .api_token
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("CLOUDFLARE_API_TOKEN").ok())
            .or_else(|| std::env::var("CF_API_TOKEN").ok())
            .or_else(detect_wrangler_token);

        let account = self
            .account_id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("CLOUDFLARE_ACCOUNT_ID").ok())
            .or_else(|| std::env::var("CF_ACCOUNT_ID").ok());

        (token, account)
    }

    pub fn to_client(&self) -> Result<CloudflareClient, FlareProxError> {
        let (token, account) = self.resolved_credentials();
        let token = token.ok_or_else(|| {
            FlareProxError::MissingCredentials(
                "Cloudflare API token not found in config or CLOUDFLARE_API_TOKEN env var"
                    .to_string(),
            )
        })?;
        let account = account.ok_or_else(|| {
            FlareProxError::MissingCredentials(
                "Cloudflare Account ID not found in config or CLOUDFLARE_ACCOUNT_ID env var"
                    .to_string(),
            )
        })?;

        CloudflareClient::new(token, account, Some(self.worker_prefix.clone()))
    }
}

/// Checks if a given proxy URL is a FlareProx gateway (e.g. *.workers.dev or scheme flareprox+https)
pub fn is_flareprox_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.starts_with("flareprox+")
        || lower.contains(".workers.dev")
        || lower.contains("flareprox")
}

/// Normalizes a FlareProx gateway URL into an HTTPS worker URL
pub fn normalize_flareprox_url(url: &str) -> String {
    let cleaned = url.strip_prefix("flareprox+").unwrap_or(url);
    if !cleaned.starts_with("http://") && !cleaned.starts_with("https://") {
        format!("https://{}", cleaned)
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_flareprox_url() {
        assert!(is_flareprox_url("https://flareprox-123.my-sub.workers.dev"));
        assert!(is_flareprox_url("flareprox+https://proxy.example.com"));
        assert!(is_flareprox_url("http://flareprox-worker/"));
        assert!(!is_flareprox_url("socks5://127.0.0.1:1080"));
        assert!(!is_flareprox_url("http://user:pass@1.2.3.4:8080"));
    }

    #[test]
    fn test_normalize_flareprox_url() {
        assert_eq!(
            normalize_flareprox_url("flareprox+https://flare.workers.dev"),
            "https://flare.workers.dev"
        );
        assert_eq!(
            normalize_flareprox_url("flare.workers.dev"),
            "https://flare.workers.dev"
        );
    }
}
