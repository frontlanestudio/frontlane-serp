use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::Deserialize;
use url::Url;

use crate::core::engine::{limit_organic_results, SearchEngine};
use crate::core::error::{Result, SerpError};
use crate::core::http_client::HttpClient;
use crate::core::types::{Query, ResultType, SearchResult};

#[derive(Clone)]
pub struct GitHub {
    http_client: HttpClient,
}

impl GitHub {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
}

#[derive(Deserialize)]
struct GhResponse {
    #[serde(default)]
    items: Vec<GhRepo>,
}

#[derive(Deserialize)]
struct GhRepo {
    full_name: String,
    html_url: String,
    description: Option<String>,
    stargazers_count: Option<u64>,
    language: Option<String>,
    forks_count: Option<u64>,
}

#[async_trait]
impl SearchEngine for GitHub {
    fn name(&self) -> &'static str {
        "github"
    }

    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>> {
        let per_page = query.limit.clamp(1, 50);
        let page = (query.start / per_page) + 1;

        let mut url =
            Url::parse("https://api.github.com/search/repositories").map_err(SerpError::Url)?;
        url.query_pairs_mut()
            .append_pair("q", &query.text)
            .append_pair("per_page", &per_page.to_string())
            .append_pair("page", &page.to_string());

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("frontlane-serp/0.1.0 (github-search)"),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github.v3+json"),
        );

        let body = self
            .http_client
            .fetch_with_headers(url.as_str(), headers)
            .await?;
        let resp: GhResponse = serde_json::from_str(&body)
            .map_err(|e| SerpError::ParserFailure(format!("Failed to parse GitHub JSON: {}", e)))?;

        let mut results = Vec::new();
        for (i, repo) in resp.items.into_iter().enumerate() {
            let stars = repo.stargazers_count.unwrap_or(0);
            let forks = repo.forks_count.unwrap_or(0);
            let lang = repo.language.unwrap_or_else(|| "Code".to_string());
            let raw_desc = repo.description.unwrap_or_default();

            let description = if raw_desc.is_empty() {
                format!("[★ {} | ⑂ {} | {}]", stars, forks, lang)
            } else {
                format!("{} [★ {} | ⑂ {} | {}]", raw_desc, stars, forks, lang)
            };

            let rank = (i + 1) as i32;
            results.push(SearchResult {
                rank,
                absolute_rank: rank,
                result_type: ResultType::Organic,
                url: repo.html_url,
                title: repo.full_name,
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
