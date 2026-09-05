use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

pub fn mask_proxy_url(raw: &str) -> String {
    if let Ok(mut u) = Url::parse(raw) {
        if u.password().is_some() {
            let _ = u.set_password(Some("***"));
        }
        return u.to_string();
    }
    raw.to_string()
}

#[derive(Debug, Clone)]
pub struct ProxyEntry {
    pub url: String,
    pub tags: Vec<String>,
    pub healthy: bool,
    pub consecutive_failures: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ProxyManager {
    inner: Arc<Mutex<ProxyManagerState>>,
}

#[derive(Debug, Default)]
struct ProxyManagerState {
    global_proxy: Option<String>,
    entries: Vec<ProxyEntry>,
    failure_threshold: usize,
    allow_request_proxy_url: bool,
}

impl ProxyManager {
    pub fn new(
        global_proxy: Option<String>,
        entries: Vec<(String, Vec<String>)>,
        failure_threshold: usize,
        allow_request_proxy_url: bool,
    ) -> Self {
        let failure_threshold = if failure_threshold == 0 { 3 } else { failure_threshold };
        let entries = entries
            .into_iter()
            .map(|(url, tags)| ProxyEntry {
                url,
                tags,
                healthy: true,
                consecutive_failures: 0,
            })
            .collect();

        Self {
            inner: Arc::new(Mutex::new(ProxyManagerState {
                global_proxy,
                entries,
                failure_threshold,
                allow_request_proxy_url,
            })),
        }
    }

    pub async fn resolve_proxy(&self, engine_tag: Option<&str>, request_proxy_url: Option<&str>) -> Option<String> {
        let state = self.inner.lock().await;
        if state.allow_request_proxy_url {
            if let Some(req_url) = request_proxy_url {
                if !req_url.is_empty() {
                    return Some(req_url.to_string());
                }
            }
        }

        if let Some(ref g) = state.global_proxy {
            if !g.is_empty() {
                return Some(g.clone());
            }
        }

        if let Some(tag) = engine_tag {
            if tag == "direct" {
                return None;
            }
            let candidate = state
                .entries
                .iter()
                .find(|e| e.healthy && e.tags.iter().any(|t| t == tag));
            if let Some(entry) = candidate {
                return Some(entry.url.clone());
            }
        }

        None
    }

    pub async fn report_result(&self, proxy_url: &str, success: bool) {
        let mut state = self.inner.lock().await;
        let threshold = state.failure_threshold;
        for entry in &mut state.entries {
            if entry.url == proxy_url {
                if success {
                    entry.consecutive_failures = 0;
                    entry.healthy = true;
                } else {
                    entry.consecutive_failures += 1;
                    if entry.consecutive_failures >= threshold {
                        entry.healthy = false;
                    }
                }
            }
        }
    }
}
