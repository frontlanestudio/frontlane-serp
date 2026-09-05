use crate::core::locale::country_from_region;
use crate::core::types::Query;
use moka::future::Cache;
use sha2::{Digest, Sha256};
use std::time::Duration;

pub fn cache_token(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn build_cache_key(engine: &str, action: &str, q: &Query) -> String {
    let country = if let Some(ref c) = q.proxy_country {
        cache_token(c)
    } else if !q.region.is_empty() {
        cache_token(&country_from_region(&q.region))
    } else {
        String::new()
    };
    let class = q
        .proxy_class
        .as_deref()
        .map(cache_token)
        .unwrap_or_default();
    let provider = q
        .proxy_provider
        .as_deref()
        .map(cache_token)
        .unwrap_or_default();

    let raw = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        cache_token(engine),
        cache_token(action),
        q.text.trim(),
        cache_token(&q.lang_code),
        cache_token(&q.region),
        q.date_interval.trim(),
        cache_token(&q.filetype),
        cache_token(&q.site),
        q.limit,
        q.start,
        q.filter,
        q.features,
        country,
        class,
        provider,
    );

    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Clone)]
pub struct ResponseCache {
    cache: Option<Cache<String, String>>,
}

impl ResponseCache {
    pub fn new(ttl_seconds: u64, max_size: u64) -> Self {
        if ttl_seconds == 0 || max_size == 0 {
            Self { cache: None }
        } else {
            let cache = Cache::builder()
                .time_to_live(Duration::from_secs(ttl_seconds))
                .max_capacity(max_size)
                .build();
            Self { cache: Some(cache) }
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        if let Some(ref c) = self.cache {
            c.get(key).await
        } else {
            None
        }
    }

    pub async fn insert(&self, key: String, value: String) {
        if let Some(ref c) = self.cache {
            c.insert(key, value).await;
        }
    }

    pub fn len(&self) -> u64 {
        if let Some(ref c) = self.cache {
            c.entry_count()
        } else {
            0
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
