use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::Deserialize;
use url::Url;

use crate::core::engine::{limit_organic_results, SearchEngine};
use crate::core::error::{Result, SerpError};
use crate::core::http_client::HttpClient;
use crate::core::types::{Query, ResultType, SearchResult};

#[derive(Clone)]
pub struct CratesIo {
    http_client: HttpClient,
}

impl CratesIo {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
}

#[derive(Deserialize)]
struct CratesResponse {
    #[serde(default)]
    crates: Vec<CrateItem>,
}

#[derive(Deserialize)]
struct CrateItem {
    name: String,
    description: Option<String>,
    max_version: Option<String>,
    downloads: Option<u64>,
}

#[async_trait]
impl SearchEngine for CratesIo {
    fn name(&self) -> &'static str {
        "crates"
    }

    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>> {
        let per_page = query.limit.clamp(1, 50);
        let page = (query.start / per_page) + 1;

        let mut url = Url::parse("https://crates.io/api/v1/crates").map_err(SerpError::Url)?;
        url.query_pairs_mut()
            .append_pair("q", &query.text)
            .append_pair("per_page", &per_page.to_string())
            .append_pair("page", &page.to_string());

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("frontlane-serp/0.1.0 (crates-search)"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let body = self
            .http_client
            .fetch_with_headers(url.as_str(), headers)
            .await?;
        let resp: CratesResponse = serde_json::from_str(&body).map_err(|e| {
            SerpError::ParserFailure(format!("Failed to parse Crates.io JSON: {}", e))
        })?;

        let mut results = Vec::new();
        for (i, c) in resp.crates.into_iter().enumerate() {
            let version = c.max_version.unwrap_or_else(|| "0.1.0".to_string());
            let downloads = c.downloads.unwrap_or(0);
            let raw_desc = c.description.unwrap_or_default();

            let description = if raw_desc.is_empty() {
                format!("Rust crate v{}, {} total downloads", version, downloads)
            } else {
                format!("{} (v{}, {} downloads)", raw_desc, version, downloads)
            };

            let rank = (i + 1) as i32;
            results.push(SearchResult {
                rank,
                absolute_rank: rank,
                result_type: ResultType::Organic,
                url: format!("https://crates.io/crates/{}", c.name),
                title: c.name,
                description,
                ad: false,
                features: vec![],
                image_data: None,
                image_source: None,
            });
        }

        Ok(limit_organic_results(results, query.limit))
    }

    async fn search_image(&self, _query: &Query) -> Result<Vec<SearchResult>> {
        Ok(vec![])
    }
}
