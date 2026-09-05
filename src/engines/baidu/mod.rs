pub mod features;
pub mod parser;
pub mod selectors;
pub mod url;

use crate::core::engine::{limit_organic_results, SearchEngine};
use crate::core::error::Result;
use crate::core::http_client::HttpClient;
use crate::core::types::{Query, SearchResult};
use async_trait::async_trait;

#[derive(Clone)]
pub struct Baidu {
    http_client: HttpClient,
}

impl Baidu {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
}

#[async_trait]
impl SearchEngine for Baidu {
    fn name(&self) -> &'static str {
        "baidu"
    }

    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>> {
        let fetch_url = url::build_url(query)?;
        let page = (query.start / 10) as i32;

        let lang = if !query.lang_code.is_empty() {
            Some(query.lang_code.as_str())
        } else {
            None
        };

        let html = self.http_client.fetch(&fetch_url, lang).await?;
        let results = parser::parse_html(&html, page)?;
        Ok(limit_organic_results(results, query.limit))
    }

    async fn search_image(&self, query: &Query) -> Result<Vec<SearchResult>> {
        let page = query.start / 10;
        let image_url = url::build_image_url(query, page)?;

        let lang = if !query.lang_code.is_empty() {
            Some(query.lang_code.as_str())
        } else {
            None
        };

        let _ = self.http_client.fetch(&image_url, lang).await?;
        Ok(Vec::new())
    }
}
