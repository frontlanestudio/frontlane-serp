use async_trait::async_trait;
use serde::Deserialize;
use url::Url;

use crate::core::engine::{limit_organic_results, SearchEngine};
use crate::core::error::{Result, SerpError};
use crate::core::http_client::HttpClient;
use crate::core::types::{Query, ResultType, SearchResult};

#[derive(Clone)]
pub struct HackerNews {
    http_client: HttpClient,
}

impl HackerNews {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
}

#[derive(Deserialize)]
struct HnResponse {
    #[serde(default)]
    hits: Vec<HnHit>,
}

#[derive(Deserialize)]
struct HnHit {
    #[serde(rename = "objectID")]
    object_id: String,
    title: Option<String>,
    story_title: Option<String>,
    url: Option<String>,
    story_url: Option<String>,
    author: Option<String>,
    points: Option<i64>,
    num_comments: Option<i64>,
    story_text: Option<String>,
    comment_text: Option<String>,
}

#[async_trait]
impl SearchEngine for HackerNews {
    fn name(&self) -> &'static str {
        "hackernews"
    }

    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>> {
        let hits_per_page = query.limit.clamp(1, 50);
        let page = query.start / hits_per_page;

        let mut url = Url::parse("https://hn.algolia.com/api/v1/search").map_err(SerpError::Url)?;
        url.query_pairs_mut()
            .append_pair("query", &query.text)
            .append_pair("hitsPerPage", &hits_per_page.to_string())
            .append_pair("page", &page.to_string());

        let body = self.http_client.fetch(url.as_str(), None).await?;
        let resp: HnResponse = serde_json::from_str(&body)
            .map_err(|e| SerpError::ParserFailure(format!("Failed to parse HN JSON: {}", e)))?;

        let mut results = Vec::new();
        for (i, hit) in resp.hits.into_iter().enumerate() {
            let title = hit
                .title
                .or(hit.story_title)
                .unwrap_or_else(|| "Hacker News Post".to_string());

            let target_url = hit.url.or(hit.story_url).unwrap_or_else(|| {
                format!("https://news.ycombinator.com/item?id={}", hit.object_id)
            });

            let points = hit.points.unwrap_or(0);
            let comments = hit.num_comments.unwrap_or(0);
            let author = hit.author.unwrap_or_else(|| "unknown".to_string());

            let mut desc = format!("{} points by {} | {} comments", points, author, comments);
            if let Some(text) = hit.story_text.or(hit.comment_text) {
                if !text.is_empty() {
                    let cleaned: String = text.chars().take(200).collect();
                    desc = format!("{} - {}", desc, cleaned);
                }
            }

            let rank = (i + 1) as i32;
            results.push(SearchResult {
                rank,
                absolute_rank: rank,
                result_type: ResultType::Organic,
                url: target_url,
                title,
                description: desc,
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
