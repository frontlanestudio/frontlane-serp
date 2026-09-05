use std::collections::HashMap;
use std::sync::Arc;

use crate::config::AppConfig;
use crate::core::cache::ResponseCache;
use crate::core::engine::SearchEngine;
use crate::core::http_client::HttpClient;
use crate::extract::Extractor;
use crate::mega::MegaSearcher;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub http_client: HttpClient,
    pub cache: Arc<ResponseCache>,
    pub engines: HashMap<String, Arc<dyn SearchEngine>>,
    pub mega: Arc<MegaSearcher>,
    pub extractor: Arc<Extractor>,
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

        let extractor = Arc::new(Extractor::new(http_client.clone()));
        let mega = Arc::new(MegaSearcher::new(engines, Some((*extractor).clone())));
        let cache = Arc::new(ResponseCache::new(
            config.cache.ttl_seconds,
            config.cache.max_size,
        ));

        Self {
            config: Arc::new(config),
            http_client,
            cache,
            engines: engine_map,
            mega,
            extractor,
        }
    }
}
