pub mod parser;
pub mod selectors;
pub mod url;

use async_trait::async_trait;
use crate::core::engine::{limit_organic_results, SearchEngine};
use crate::core::error::Result;
use crate::core::http_client::HttpClient;
use crate::core::types::{Query, SearchResult};

#[derive(Clone)]
pub struct Ecosia {
    http_client: HttpClient,
}

impl Ecosia {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
}

#[async_trait]
impl SearchEngine for Ecosia {
    fn name(&self) -> &'static str {
        "ecosia"
    }

    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>> {
        let page = query.start / 10;
        let fetch_url = url::build_url(query, page)?;

        let lang = if !query.lang_code.is_empty() {
            Some(query.lang_code.as_str())
        } else {
            None
        };

        let html = self.http_client.fetch(&fetch_url, lang).await?;
        let results = parser::parse_html(&html)?;
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

        let html = self.http_client.fetch(&image_url, lang).await?;
        let results = parser::parse_image_html(&html)?;
        Ok(limit_organic_results(results, query.limit))
    }
}
