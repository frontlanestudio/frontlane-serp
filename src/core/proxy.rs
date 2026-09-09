use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

use crate::core::captcha::CloudflareClearance;

pub const PROXY_CHALLENGE_COOLDOWN_SECS: i64 = 120; // 2 minutes
pub const DEFAULT_MAX_LANES: usize = 100;

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
    pub consecutive_challenges: usize,
    pub challenged_until: Option<DateTime<Utc>>,
}

impl ProxyEntry {
    pub fn new(url: String, tags: Vec<String>) -> Self {
        Self {
            url,
            tags,
            healthy: true,
            consecutive_failures: 0,
            consecutive_challenges: 0,
            challenged_until: None,
        }
    }

    pub fn is_challenged(&self) -> bool {
        self.challenged_until
            .map(|t| Utc::now() < t)
            .unwrap_or(false)
    }

    pub fn matches_tag(&self, target_tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(target_tag))
    }

    pub fn matches_country(&self, country: &str) -> bool {
        let country_prefix = format!("country:{}", country);
        self.tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(country) || t.eq_ignore_ascii_case(&country_prefix))
    }
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
        let failure_threshold = if failure_threshold == 0 {
            3
        } else {
            failure_threshold
        };
        let entries = entries
            .into_iter()
            .map(|(url, tags)| ProxyEntry::new(url, tags))
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

    fn select_best_proxy<'a>(
        candidates: impl IntoIterator<Item = &'a ProxyEntry>,
    ) -> Option<String> {
        let list: Vec<&'a ProxyEntry> = candidates.into_iter().collect();
        if list.is_empty() {
            None
        } else if let Some(unchallenged) = list.iter().find(|e| !e.is_challenged()) {
            Some(unchallenged.url.clone())
        } else {
            Some(list[0].url.clone())
        }
    }

    pub async fn resolve_proxy(
        &self,
        engine_tag: Option<&str>,
        request_proxy_url: Option<&str>,
        country: Option<&str>,
    ) -> Option<String> {
        let state = self.inner.lock().await;

        // 1. Explicit request proxy URL header has highest precedence if allowed
        if state.allow_request_proxy_url {
            if let Some(req_url) = request_proxy_url {
                let trimmed = req_url.trim();
                if !trimmed.is_empty() {
                    if crate::core::network_guard::validate_public_proxy_url(trimmed)
                        .await
                        .is_ok()
                    {
                        return Some(trimmed.to_string());
                    } else {
                        tracing::warn!(proxy = %trimmed, "blocked unverified or non-public request proxy URL");
                    }
                }
            }
        }

        // 2. Direct override opts out of proxying
        if let Some(tag) = engine_tag {
            if tag.eq_ignore_ascii_case("direct") {
                return None;
            }
        }

        // 3. Geo-targeted country matching
        if let Some(c) = country {
            let clean_c = c.trim();
            if !clean_c.is_empty() {
                let country_candidates = state
                    .entries
                    .iter()
                    .filter(|e| e.healthy && e.matches_country(clean_c));
                if let Some(selected) = Self::select_best_proxy(country_candidates) {
                    return Some(selected);
                }
            }
        }

        // 4. Engine-specific tag matching
        if let Some(tag) = engine_tag {
            let tag_candidates = state
                .entries
                .iter()
                .filter(|e| e.healthy && e.matches_tag(tag));
            if let Some(selected) = Self::select_best_proxy(tag_candidates) {
                return Some(selected);
            }
        }

        // 5. Global proxy
        if let Some(ref g) = state.global_proxy {
            if !g.trim().is_empty() {
                return Some(g.clone());
            }
        }

        // 6. Any healthy proxy (preferring non-challenged)
        let healthy_candidates = state.entries.iter().filter(|e| e.healthy);
        Self::select_best_proxy(healthy_candidates)
    }

    pub async fn report_result(&self, proxy_url: &str, success: bool) {
        let mut state = self.inner.lock().await;
        let threshold = state.failure_threshold;
        for entry in &mut state.entries {
            if entry.url == proxy_url {
                if success {
                    entry.consecutive_failures = 0;
                    entry.healthy = true;
                    entry.consecutive_challenges = 0;
                    entry.challenged_until = None;
                } else {
                    entry.consecutive_failures += 1;
                    if entry.consecutive_failures >= threshold {
                        entry.healthy = false;
                    }
                }
            }
        }
    }

    pub async fn report_challenge(&self, proxy_url: &str) {
        let mut state = self.inner.lock().await;
        for entry in &mut state.entries {
            if entry.url == proxy_url {
                entry.consecutive_challenges += 1;
                entry.challenged_until =
                    Some(Utc::now() + Duration::seconds(PROXY_CHALLENGE_COOLDOWN_SECS));
            }
        }
    }

    pub async fn stats(&self, lane_stats: Option<LaneStats>) -> ProxyStats {
        let state = self.inner.lock().await;
        let entries = state
            .entries
            .iter()
            .map(|e| ProxyStatsEntry {
                url: mask_proxy_url(&e.url),
                tags: e.tags.clone(),
                healthy: e.healthy,
                consecutive_failures: e.consecutive_failures,
                consecutive_challenges: Some(e.consecutive_challenges),
            })
            .collect();

        ProxyStats {
            allow_request_proxy_url: state.allow_request_proxy_url,
            entries,
            lanes: lane_stats,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaneStats {
    pub active: usize,
    pub evicted_lru: usize,
    pub cookies_dropped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatsEntry {
    pub url: String,
    pub tags: Vec<String>,
    pub healthy: bool,
    pub consecutive_failures: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consecutive_challenges: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyStats {
    pub allow_request_proxy_url: bool,
    pub entries: Vec<ProxyStatsEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lanes: Option<LaneStats>,
}

// --- Proxy Lanes for Sticky Session Modeling ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyLaneKey {
    pub tenant: String,
    pub engine: String,
    pub session_id: String,
}

impl ProxyLaneKey {
    pub fn new(
        tenant: impl Into<String>,
        engine: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            tenant: tenant.into(),
            engine: engine.into(),
            session_id: session_id.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.session_id.trim().is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct LaneState {
    pub key: ProxyLaneKey,
    pub proxy_url: Option<String>,
    pub user_agent: String,
    pub clearances: HashMap<String, CloudflareClearance>,
    pub last_used_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LaneStore {
    inner: Arc<Mutex<HashMap<ProxyLaneKey, LaneState>>>,
    max_lanes: usize,
}

impl Default for LaneStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_LANES)
    }
}

impl LaneStore {
    pub fn new(max_lanes: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max_lanes: if max_lanes == 0 {
                DEFAULT_MAX_LANES
            } else {
                max_lanes
            },
        }
    }

    pub async fn get_or_create_lane(
        &self,
        key: ProxyLaneKey,
        default_proxy: Option<String>,
        default_ua: String,
    ) -> LaneState {
        let mut map = self.inner.lock().await;

        if let Some(lane) = map.get_mut(&key) {
            lane.last_used_at = Utc::now();
            return lane.clone();
        }

        // LRU eviction if at capacity
        if map.len() >= self.max_lanes {
            if let Some(oldest_key) = map
                .iter()
                .min_by_key(|(_, v)| v.last_used_at)
                .map(|(k, _)| k.clone())
            {
                map.remove(&oldest_key);
            }
        }

        let new_lane = LaneState {
            key: key.clone(),
            proxy_url: default_proxy,
            user_agent: default_ua,
            clearances: HashMap::new(),
            last_used_at: Utc::now(),
        };

        map.insert(key, new_lane.clone());
        new_lane
    }

    pub async fn get_clearance(
        &self,
        key: &ProxyLaneKey,
        domain: &str,
    ) -> Option<CloudflareClearance> {
        let map = self.inner.lock().await;
        if let Some(lane) = map.get(key) {
            if let Some(clearance) = lane.clearances.get(domain) {
                if !clearance.is_expired() {
                    return Some(clearance.clone());
                }
            }
        }
        None
    }

    pub async fn set_clearance(&self, key: &ProxyLaneKey, clearance: CloudflareClearance) {
        let mut map = self.inner.lock().await;
        if let Some(lane) = map.get_mut(key) {
            lane.clearances.insert(clearance.domain.clone(), clearance);
            lane.last_used_at = Utc::now();
        }
    }

    pub async fn drop_clearance_on_challenge(&self, key: &ProxyLaneKey, domain: &str) {
        let mut map = self.inner.lock().await;
        if let Some(lane) = map.get_mut(key) {
            lane.clearances.remove(domain);
        }
    }

    pub async fn stats(&self) -> LaneStats {
        let map = self.inner.lock().await;
        LaneStats {
            active: map.len(),
            evicted_lru: 0,
            cookies_dropped: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_geo_targeted_proxy_routing() {
        let entries = vec![
            (
                "http://proxy-us:8080".to_string(),
                vec!["us".to_string(), "residential".to_string()],
            ),
            (
                "http://proxy-de:8080".to_string(),
                vec!["country:de".to_string(), "datacenter".to_string()],
            ),
            (
                "http://proxy-default:8080".to_string(),
                vec!["default".to_string()],
            ),
        ];

        let pm = ProxyManager::new(None, entries, 3, false);

        // US routing
        let us_proxy = pm.resolve_proxy(None, None, Some("US")).await;
        assert_eq!(us_proxy, Some("http://proxy-us:8080".to_string()));

        // DE routing
        let de_proxy = pm.resolve_proxy(None, None, Some("de")).await;
        assert_eq!(de_proxy, Some("http://proxy-de:8080".to_string()));

        // Fallback when unknown country
        let jp_proxy = pm.resolve_proxy(Some("default"), None, Some("jp")).await;
        assert_eq!(jp_proxy, Some("http://proxy-default:8080".to_string()));
    }

    #[tokio::test]
    async fn test_proxy_challenge_cooldown() {
        let entries = vec![
            ("http://proxy-1:8080".to_string(), vec!["pool".to_string()]),
            ("http://proxy-2:8080".to_string(), vec!["pool".to_string()]),
        ];

        let pm = ProxyManager::new(None, entries, 3, false);

        // Report challenge on proxy 1
        pm.report_challenge("http://proxy-1:8080").await;

        // proxy 2 should be preferred because proxy 1 is challenged
        let selected = pm.resolve_proxy(Some("pool"), None, None).await;
        assert_eq!(selected, Some("http://proxy-2:8080".to_string()));
    }

    #[tokio::test]
    async fn test_lane_store_clearance_lifecycle() {
        let store = LaneStore::new(10);
        let key = ProxyLaneKey::new("tenant1", "google", "session123");

        let lane = store
            .get_or_create_lane(
                key.clone(),
                Some("http://proxy:8080".to_string()),
                "Custom UA".to_string(),
            )
            .await;
        assert_eq!(lane.user_agent, "Custom UA");

        let clearance = CloudflareClearance::new(
            "protected-shop.com",
            "cf_clearance_abc",
            "Custom UA",
            100,
            Some("http://proxy:8080".to_string()),
        );

        store.set_clearance(&key, clearance.clone()).await;

        let cached = store.get_clearance(&key, "protected-shop.com").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().cf_clearance, "cf_clearance_abc");

        store
            .drop_clearance_on_challenge(&key, "protected-shop.com")
            .await;
        assert!(store
            .get_clearance(&key, "protected-shop.com")
            .await
            .is_none());
    }
}
