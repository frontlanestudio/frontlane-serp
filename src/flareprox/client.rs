use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::flareprox::worker_script::FLAREPROX_WORKER_JS;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";
const DEFAULT_PREFIX: &str = "flareprox";

#[derive(Debug, thiserror::Error)]
pub enum FlareProxError {
    #[error("Missing Cloudflare credentials: {0}")]
    MissingCredentials(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Cloudflare API error: {0}")]
    Api(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Worker timeout: {0}")]
    Timeout(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlareProxDeployment {
    pub name: String,
    pub url: String,
    pub created_at: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlareProxTestResult {
    pub name: String,
    pub url: String,
    pub target_url: String,
    pub status: u16,
    pub latency_ms: u128,
    pub egress_ip: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct CloudflareClient {
    client: Client,
    api_token: String,
    account_id: String,
    worker_prefix: String,
}

#[derive(Deserialize)]
struct CfResponse<T> {
    success: bool,
    errors: Option<Vec<CfErrorItem>>,
    result: Option<T>,
}

#[derive(Deserialize)]
struct CfErrorItem {
    #[allow(dead_code)]
    code: Option<i64>,
    message: String,
}

#[derive(Deserialize)]
struct SubdomainResult {
    subdomain: Option<String>,
}

#[derive(Deserialize)]
struct ScriptItem {
    id: String,
    created_on: Option<String>,
}

#[derive(Deserialize)]
struct AccountItem {
    id: String,
    name: String,
}

pub fn resolve_placement(region: &str) -> Option<String> {
    let trimmed = region.trim().to_lowercase();
    if trimmed.starts_with("aws:") || trimmed.starts_with("gcp:") || trimmed.starts_with("azure:") {
        return Some(trimmed);
    }
    match trimmed.as_str() {
        "de" | "germany" | "frankfurt" => Some("aws:eu-central-1".to_string()),
        "gb" | "uk" | "london" => Some("aws:eu-west-2".to_string()),
        "ie" | "ireland" | "dublin" => Some("aws:eu-west-1".to_string()),
        "us" | "usa" | "us-east" | "virginia" => Some("aws:us-east-1".to_string()),
        "us-west" | "oregon" | "california" => Some("aws:us-west-2".to_string()),
        "jp" | "japan" | "tokyo" => Some("aws:ap-northeast-1".to_string()),
        "sg" | "singapore" => Some("aws:ap-southeast-1".to_string()),
        "au" | "australia" | "sydney" => Some("aws:ap-southeast-2".to_string()),
        "fr" | "france" | "paris" => Some("aws:eu-west-3".to_string()),
        "br" | "brazil" | "saopaulo" => Some("aws:sa-east-1".to_string()),
        "in" | "india" | "mumbai" => Some("aws:ap-south-1".to_string()),
        "ca" | "canada" | "central" => Some("aws:ca-central-1".to_string()),
        _ => None,
    }
}

impl CloudflareClient {
    pub async fn resolve_account_id(api_token: &str) -> Result<String, FlareProxError> {
        let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
        let resp = client
            .get(format!("{}/accounts", CF_API_BASE))
            .bearer_auth(api_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(FlareProxError::Api(format!(
                "Failed to query accounts: HTTP {}",
                resp.status()
            )));
        }

        let cf_res: CfResponse<Vec<AccountItem>> = resp.json().await?;
        let accounts = cf_res.result.unwrap_or_default();
        if accounts.is_empty() {
            return Err(FlareProxError::Api(
                "No accessible Cloudflare accounts found for this token".to_string(),
            ));
        }

        if let Some(frontlane) = accounts
            .iter()
            .find(|a| a.name.to_lowercase().contains("frontlane"))
        {
            return Ok(frontlane.id.clone());
        }

        Ok(accounts[0].id.clone())
    }
    pub fn new(
        api_token: String,
        account_id: String,
        worker_prefix: Option<String>,
    ) -> Result<Self, FlareProxError> {
        if api_token.trim().is_empty() {
            return Err(FlareProxError::MissingCredentials(
                "API token is empty. Provide via config or CLOUDFLARE_API_TOKEN env var."
                    .to_string(),
            ));
        }
        if account_id.trim().is_empty() {
            return Err(FlareProxError::MissingCredentials(
                "Account ID is empty. Provide via config or CLOUDFLARE_ACCOUNT_ID env var."
                    .to_string(),
            ));
        }

        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

        let worker_prefix = worker_prefix
            .unwrap_or_else(|| DEFAULT_PREFIX.to_string())
            .trim()
            .to_string();

        Ok(Self {
            client,
            api_token,
            account_id,
            worker_prefix,
        })
    }

    pub fn worker_prefix(&self) -> &str {
        &self.worker_prefix
    }

    pub async fn get_subdomain(&self) -> Result<Option<String>, FlareProxError> {
        let url = format!(
            "{}/accounts/{}/workers/subdomain",
            CF_API_BASE, self.account_id
        );
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let cf_res: CfResponse<SubdomainResult> = resp.json().await?;
        if cf_res.success {
            Ok(cf_res.result.and_then(|r| r.subdomain))
        } else {
            let msg = cf_res
                .errors
                .and_then(|e| e.into_iter().next())
                .map(|e| e.message)
                .unwrap_or_else(|| "Unknown Cloudflare API error".to_string());
            Err(FlareProxError::Api(msg))
        }
    }

    pub async fn provision_subdomain(&self) -> Result<String, FlareProxError> {
        let url = format!(
            "{}/accounts/{}/workers/subdomain",
            CF_API_BASE, self.account_id
        );
        let prefix = &self.account_id[..std::cmp::min(10, self.account_id.len())].to_lowercase();
        let random_part: String = (0..4)
            .map(|_| {
                let idx = (rand_u32() % 26) as u8;
                (b'a' + idx) as char
            })
            .collect();
        let desired_name = format!("{}-{}", prefix, random_part);

        let resp = self
            .client
            .put(&url)
            .bearer_auth(&self.api_token)
            .json(&serde_json::json!({ "subdomain": desired_name }))
            .send()
            .await?;

        let cf_res: CfResponse<SubdomainResult> = resp.json().await?;
        if cf_res.success {
            if let Some(sub) = cf_res.result.and_then(|r| r.subdomain) {
                return Ok(sub);
            }
        }

        // If 409 or already exists, try to get it again
        if let Some(existing) = self.get_subdomain().await? {
            return Ok(existing);
        }

        let msg = cf_res
            .errors
            .and_then(|e| e.into_iter().next())
            .map(|e| e.message)
            .unwrap_or_else(|| "Failed to provision workers.dev subdomain".to_string());
        Err(FlareProxError::Api(msg))
    }

    pub async fn ensure_subdomain(&self) -> Result<String, FlareProxError> {
        if let Some(sub) = self.get_subdomain().await? {
            if !sub.trim().is_empty() {
                return Ok(sub);
            }
        }
        self.provision_subdomain().await
    }

    pub fn generate_worker_name(&self, region: Option<&str>) -> String {
        let now = chrono::Utc::now().timestamp();
        let random_suffix: String = (0..6)
            .map(|_| {
                let idx = (rand_u32() % 26) as u8;
                (b'a' + idx) as char
            })
            .collect();
        if let Some(r) = region {
            let clean_r = r.trim().to_lowercase().replace(':', "-");
            format!("{}-{}-{}-{}", self.worker_prefix, clean_r, now, random_suffix)
        } else {
            format!("{}-{}-{}", self.worker_prefix, now, random_suffix)
        }
    }

    pub async fn create_worker(
        &self,
        name: Option<&str>,
        region: Option<&str>,
    ) -> Result<FlareProxDeployment, FlareProxError> {
        let subdomain = self.ensure_subdomain().await?;
        let worker_name = match name {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => self.generate_worker_name(region),
        };

        let upload_url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CF_API_BASE, self.account_id, worker_name
        );

        let mut metadata = serde_json::json!({
            "body_part": "script",
            "main_module": "worker.js"
        });

        if let Some(r) = region {
            if let Some(placement) = resolve_placement(r) {
                metadata["placement"] = serde_json::json!({
                    "mode": "smart",
                    "region": placement
                });
            }
        }

        let metadata_part = Part::text(metadata.to_string())
            .mime_str("application/json")
            .map_err(|e| FlareProxError::Api(e.to_string()))?;

        let script_part = Part::text(FLAREPROX_WORKER_JS)
            .file_name("worker.js")
            .mime_str("application/javascript")
            .map_err(|e| FlareProxError::Api(e.to_string()))?;

        let form = Form::new()
            .part("metadata", metadata_part)
            .part("script", script_part);

        let resp = self
            .client
            .put(&upload_url)
            .bearer_auth(&self.api_token)
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(FlareProxError::Api(format!(
                "Failed to upload worker script: {}",
                err_text
            )));
        }

        // Enable on subdomain
        let subdomain_url = format!(
            "{}/accounts/{}/workers/scripts/{}/subdomain",
            CF_API_BASE, self.account_id, worker_name
        );

        let mut enabled = false;
        for _ in 0..3 {
            let sub_resp = self
                .client
                .post(&subdomain_url)
                .bearer_auth(&self.api_token)
                .json(&serde_json::json!({ "enabled": true }))
                .send()
                .await;

            if let Ok(res) = sub_resp {
                if res.status().is_success() {
                    enabled = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if !enabled {
            warn!(
                "Worker uploaded but could not confirm subdomain enablement for {}",
                worker_name
            );
        }

        let worker_url = format!("https://{}.{}.workers.dev", worker_name, subdomain);
        Ok(FlareProxDeployment {
            name: worker_name.clone(),
            url: worker_url,
            created_at: chrono::Utc::now().to_rfc3339(),
            id: worker_name,
        })
    }

    pub async fn deploy_main_edge_worker(
        &self,
        worker_name: Option<&str>,
        proxy_urls: &[String],
        auto_recycle: bool,
    ) -> Result<FlareProxDeployment, FlareProxError> {
        let subdomain = self.ensure_subdomain().await?;
        let name = worker_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "frontlane-serp-edge".to_string());

        let upload_url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CF_API_BASE, self.account_id, name
        );

        let proxy_pool_str = proxy_urls.join(",");
        let mut bindings = vec![
            serde_json::json!({
                "name": "PROXY_POOL",
                "type": "plain_text",
                "text": proxy_pool_str
            }),
            serde_json::json!({
                "name": "AUTO_RECYCLE",
                "type": "plain_text",
                "text": if auto_recycle { "true" } else { "false" }
            }),
        ];

        if auto_recycle {
            bindings.push(serde_json::json!({
                "name": "CF_API_TOKEN",
                "type": "secret_text",
                "text": self.api_token
            }));
            bindings.push(serde_json::json!({
                "name": "CF_ACCOUNT_ID",
                "type": "plain_text",
                "text": self.account_id
            }));
        }

        let metadata = serde_json::json!({
            "body_part": "script",
            "main_module": "worker.js",
            "bindings": bindings
        });

        let metadata_part = Part::text(metadata.to_string())
            .mime_str("application/json")
            .map_err(|e| FlareProxError::Api(e.to_string()))?;

        let script_part = Part::text(crate::flareprox::edge_worker_script::EDGE_WORKER_JS)
            .file_name("worker.js")
            .mime_str("application/javascript")
            .map_err(|e| FlareProxError::Api(e.to_string()))?;

        let form = Form::new()
            .part("metadata", metadata_part)
            .part("script", script_part);

        let resp = self
            .client
            .put(&upload_url)
            .bearer_auth(&self.api_token)
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(FlareProxError::Api(format!(
                "Failed to upload main edge worker script: {}",
                err_text
            )));
        }

        // Enable on subdomain
        let subdomain_url = format!(
            "{}/accounts/{}/workers/scripts/{}/subdomain",
            CF_API_BASE, self.account_id, name
        );

        for _ in 0..3 {
            let sub_resp = self
                .client
                .post(&subdomain_url)
                .bearer_auth(&self.api_token)
                .json(&serde_json::json!({ "enabled": true }))
                .send()
                .await;

            if let Ok(res) = sub_resp {
                if res.status().is_success() {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let worker_url = format!("https://{}.{}.workers.dev", name, subdomain);
        Ok(FlareProxDeployment {
            name: name.clone(),
            url: worker_url,
            created_at: chrono::Utc::now().to_rfc3339(),
            id: name,
        })
    }

    pub async fn list_workers(&self) -> Result<Vec<FlareProxDeployment>, FlareProxError> {
        let subdomain = self.get_subdomain().await?.unwrap_or_default();
        let url = format!(
            "{}/accounts/{}/workers/scripts",
            CF_API_BASE, self.account_id
        );

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(FlareProxError::Api(format!(
                "Failed to list workers: {}",
                err
            )));
        }

        let cf_res: CfResponse<Vec<ScriptItem>> = resp.json().await?;
        let scripts = cf_res.result.unwrap_or_default();

        let prefix_match = format!("{}-", self.worker_prefix);
        let mut deployments = Vec::new();

        for s in scripts {
            if s.id.starts_with(&prefix_match) || s.id == self.worker_prefix {
                let worker_url = if !subdomain.is_empty() {
                    format!("https://{}.{}.workers.dev", s.id, subdomain)
                } else {
                    format!("https://{}.workers.dev", s.id)
                };

                deployments.push(FlareProxDeployment {
                    name: s.id.clone(),
                    url: worker_url,
                    created_at: s.created_on.unwrap_or_else(|| "unknown".to_string()),
                    id: s.id,
                });
            }
        }

        Ok(deployments)
    }

    pub async fn delete_worker(&self, name: &str) -> Result<bool, FlareProxError> {
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CF_API_BASE, self.account_id, name
        );

        let resp = self
            .client
            .delete(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(FlareProxError::Api(format!(
                "Failed to delete worker {}: {}",
                name, err
            )));
        }

        Ok(true)
    }

    pub async fn cleanup_all(&self) -> Result<usize, FlareProxError> {
        let workers = self.list_workers().await?;
        let mut count = 0;
        for w in workers {
            match self.delete_worker(&w.name).await {
                Ok(true) => {
                    info!("Deleted FlareProx worker: {}", w.name);
                    count += 1;
                }
                Ok(false) => {}
                Err(e) => {
                    warn!("Failed to delete worker {}: {}", w.name, e);
                }
            }
        }
        Ok(count)
    }

    pub async fn test_endpoint(
        &self,
        worker: &FlareProxDeployment,
        target_url: &str,
    ) -> FlareProxTestResult {
        let start = Instant::now();

        let request = self
            .client
            .get(&worker.url)
            .header("X-Target-URL", target_url)
            .timeout(Duration::from_secs(15));

        match request.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let latency_ms = start.elapsed().as_millis();
                let body = resp.text().await.unwrap_or_default();
                let clean_body = body.trim().to_string();

                let is_ip_like =
                    clean_body.len() < 50 && (clean_body.contains('.') || clean_body.contains(':'));

                let egress_ip = if is_ip_like { Some(clean_body) } else { None };

                let success = (200..400).contains(&status);

                FlareProxTestResult {
                    name: worker.name.clone(),
                    url: worker.url.clone(),
                    target_url: target_url.to_string(),
                    status,
                    latency_ms,
                    egress_ip,
                    success,
                    error: if !success {
                        Some(format!("HTTP {}", status))
                    } else {
                        None
                    },
                }
            }
            Err(e) => FlareProxTestResult {
                name: worker.name.clone(),
                url: worker.url.clone(),
                target_url: target_url.to_string(),
                status: 0,
                latency_ms: start.elapsed().as_millis(),
                egress_ip: None,
                success: false,
                error: Some(e.to_string()),
            },
        }
    }
}

/// Simple pseudo-random u32 generator using timestamp & memory address to avoid adding heavy rand crate
fn rand_u32() -> u32 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(42);
    let ptr = &seed as *const _ as usize as u32;
    seed.wrapping_mul(1664525)
        .wrapping_add(ptr)
        .wrapping_add(1013904223)
}
