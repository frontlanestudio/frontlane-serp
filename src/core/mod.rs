pub mod cache;
pub mod captcha;
pub mod circuit_breaker;
pub mod clusters;
pub mod domain;
pub mod engine;
pub mod error;
pub mod feature_selectors;
pub mod format;
pub mod http_client;
pub mod locale;
pub mod network_guard;
pub mod proxy;
pub mod rate_limiter;
pub mod resilient;
pub mod response_builder;
pub mod types;

pub use feature_selectors::{
    attach_features_to_results, deduplicate_serp_features, extract_serp_features_by_selectors,
    SerpFeatureSelector,
};

pub use captcha::{CaptchaSolver, CaptchaSolverConfig, CloudflareClearance};
pub use clusters::build_clusters;
pub use domain::{classify_url, enrich_domain_info, normalize_domain, split_domain};
pub use engine::{count_organic_results, deduplicate_results, limit_organic_results, SearchEngine};
pub use error::{Result, SerpError};
pub use format::{render_envelope, render_image_envelope};
pub use http_client::HttpClient;
pub use locale::{
    build_accept_language_header, country_from_region, google_uule, parse_locale, resolve_region,
    yandex_lr,
};
pub use proxy::{mask_proxy_url, LaneState, LaneStore, ProxyEntry, ProxyLaneKey, ProxyManager};
pub use rate_limiter::RateLimiter;
pub use resilient::ResilientSearcher;
pub use response_builder::{
    build_feature_id, build_image_id, build_result_id, compute_result_position,
    enrich_image_result, enrich_result, enrich_serp_feature, normalize_url,
};
pub use types::*;

pub mod page_helpers;
pub use page_helpers::{classify_challenge_document, normalize_whitespace, DocSignals, RankState};
