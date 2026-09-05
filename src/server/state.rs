use std::collections::HashMap;
use std::sync::Arc;

use crate::config::AppConfig;
use crate::core::cache::ResponseCache;
use crate::core::captcha::{CaptchaSolver, CaptchaSolverConfig};
use crate::core::circuit_breaker::{CircuitBreakerConfig as CbConfig, CircuitBreakerManager};
use crate::core::engine::SearchEngine;
use crate::core::http_client::HttpClient;
use crate::core::proxy::{LaneStore, ProxyManager};
use crate::extract::Extractor;
use crate::jobs::JobManager;
use crate::mega::MegaSearcher;
use crate::suggest::SuggestClient;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub http_client: HttpClient,
    pub cache: Arc<ResponseCache>,
    pub engines: HashMap<String, Arc<dyn SearchEngine>>,
    pub mega: Arc<MegaSearcher>,
    pub extractor: Arc<Extractor>,
    pub suggest: Arc<SuggestClient>,
    pub jobs: Arc<JobManager>,
    pub proxy_manager: Arc<ProxyManager>,
    pub lane_store: Arc<LaneStore>,
    pub captcha_solver: Arc<CaptchaSolver>,
    pub circuit_breaker_manager: Arc<CircuitBreakerManager>,
    pub crawler: Arc<crate::crawl::Crawler>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        engines: Vec<Arc<dyn SearchEngine>>,
        http_client: HttpClient,
    ) -> Self {
        let mut engine_map = HashMap::new();
        for e in &engines {
            engine_map.insert(e.name().to_string(), e.clone());
        }

        let entries: Vec<(String, Vec<String>)> = config
            .proxies
            .entries
            .iter()
            .map(|e| (e.url.clone(), e.tags.clone()))
            .collect();

        let proxy_manager = Arc::new(ProxyManager::new(
            config.proxies.global.clone(),
            entries,
            config.proxies.health.failure_threshold as usize,
            config.proxies.allow_request_proxy_url,
        ));

        let lane_store = Arc::new(LaneStore::new(config.proxies.lanes.max_lanes));

        let solver_config = CaptchaSolverConfig {
            enabled: config.captcha.solver_enabled,
            provider: config.captcha.provider.clone(),
            api_key: config.captcha.apikey.clone().unwrap_or_default(),
            poll_interval_ms: 2000,
            max_poll_timeout_secs: 60,
        };
        let captcha_solver = Arc::new(CaptchaSolver::new(solver_config));

        let extractor = Arc::new(Extractor::with_solver_and_lanes(
            http_client.clone(),
            Some((*captcha_solver).clone()),
            Some((*lane_store).clone()),
        ));

        let mega = Arc::new(MegaSearcher::new(engines, Some((*extractor).clone())));
        let cache = Arc::new(ResponseCache::new(
            config.cache.ttl_seconds,
            config.cache.max_size,
        ));
        let suggest = Arc::new(SuggestClient::new(http_client.clone()));
        let jobs = Arc::new(JobManager::new(http_client.clone()));

        let cb_cfg = if let Some(ref cb) = config.circuit_breaker {
            CbConfig {
                failure_threshold: cb.failures as usize,
                recovery_duration: std::time::Duration::from_secs(cb.recovery_seconds),
                success_threshold: cb.successes as usize,
            }
        } else {
            CbConfig::default()
        };
        let circuit_breaker_manager = Arc::new(CircuitBreakerManager::new(cb_cfg));

        let crawler = Arc::new(crate::crawl::Crawler::new(
            http_client.clone(),
            extractor.clone(),
        ));

        Self {
            config: Arc::new(config),
            http_client,
            cache,
            engines: engine_map,
            mega,
            extractor,
            suggest,
            jobs,
            proxy_manager,
            lane_store,
            captcha_solver,
            circuit_breaker_manager,
            crawler,
        }
    }
}
