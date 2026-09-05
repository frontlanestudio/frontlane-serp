use crate::core::types::SerpFeature;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RankStrategy {
    #[default]
    Smart,
    Basic,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DomainMatchMode {
    #[default]
    Subdomain,
    Exact,
    Wildcard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    #[default]
    Desktop,
    Mobile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RankRequest {
    pub target: String,
    pub q: String,
    #[serde(default)]
    pub strategy: RankStrategy,
    #[serde(default)]
    pub last_rank: usize,
    #[serde(default = "default_pagination_limit")]
    pub pagination_limit: usize,
    #[serde(default)]
    pub smart_full_fallback: bool,
    #[serde(default)]
    pub r#match: DomainMatchMode,
    #[serde(default)]
    pub device: DeviceType,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub lang: String,
}

fn default_pagination_limit() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankResponse {
    pub target: String,
    pub query: String,
    pub engine: String,
    pub device: String,
    pub ranked: bool,
    pub rank: Option<usize>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub serp_features: Vec<SerpFeature>,
    pub feature_citations: Vec<String>,
    pub pages_scraped: Vec<usize>,
    pub took_ms: i64,
}
