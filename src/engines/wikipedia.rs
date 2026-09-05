use async_trait::async_trait;
use url::Url;

use crate::core::engine::{limit_organic_results, SearchEngine};
use crate::core::error::{Result, SerpError};
use crate::core::http_client::HttpClient;
use crate::core::types::{Query, ResultType, SearchResult};

#[derive(Clone)]
pub struct Wikipedia {
    http_client: HttpClient,
}

impl Wikipedia {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
}

#[async_trait]
impl SearchEngine for Wikipedia {
    fn name(&self) -> &'static str {
        "wikipedia"
    }

    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>> {
        let lang = if query.lang_code.is_empty() {
            "en"
        } else {
            query.lang_code.as_str()
        };

        let base_url = format!("https://{}.wikipedia.org/w/api.php", lang);
        let mut url = Url::parse(&base_url).map_err(SerpError::Url)?;
        url.query_pairs_mut()
            .append_pair("action", "opensearch")
            .append_pair("search", &query.text)
            .append_pair("limit", &query.limit.to_string())
            .append_pair("namespace", "0")
            .append_pair("format", "json");

        let body = self.http_client.fetch(url.as_str(), Some(lang)).await?;
        let parsed: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            SerpError::ParserFailure(format!("Failed to parse Wikipedia JSON: {}", e))
        })?;

        let arr = parsed.as_array().ok_or_else(|| {
            SerpError::ParserFailure("Expected JSON array from Wikipedia".to_string())
        })?;

        if arr.len() < 4 {
            return Ok(vec![]);
        }

        let empty_vec = vec![];
        let titles = arr[1].as_array().unwrap_or(&empty_vec);
        let descriptions = arr[2].as_array().unwrap_or(&empty_vec);
        let links = arr[3].as_array().unwrap_or(&empty_vec);

        let mut results = Vec::new();
        let total = titles.len().min(descriptions.len()).min(links.len());

        for i in 0..total {
            let title = titles[i].as_str().unwrap_or_default().to_string();
            let description = descriptions[i].as_str().unwrap_or_default().to_string();
            let link = links[i].as_str().unwrap_or_default().to_string();

            if title.is_empty() || link.is_empty() {
                continue;
            }

            let rank = (i + 1) as i32;
            results.push(SearchResult {
                rank,
                absolute_rank: rank,
                result_type: ResultType::Organic,
                url: link,
                title,
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
