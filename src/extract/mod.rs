pub mod llmstxt;
pub mod readability;

use chrono::Utc;
use crate::core::error::Result;
use crate::core::http_client::HttpClient;
use crate::core::types::ExtractedContent;

#[derive(Clone)]
pub struct Extractor {
    http_client: HttpClient,
}

impl Extractor {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn extract(&self, url: &str, use_llmstxt: bool) -> Result<ExtractedContent> {
        let now = Utc::now().to_rfc3339();

        if use_llmstxt {
            if let Some((_, text)) = llmstxt::try_llms_txt(&self.http_client, url).await {
                return Ok(ExtractedContent {
                    title: Some("llms.txt".to_string()),
                    format: Some("markdown".to_string()),
                    content: Some(text),
                    mode_used: Some("llmstxt".to_string()),
                    fetched_at: Some(now),
                    error: None,
                });
            }
        }

        match self.http_client.fetch(url, None).await {
            Ok(html) => {
                let page = readability::extract_page_content(&html);
                Ok(ExtractedContent {
                    title: if !page.title.is_empty() { Some(page.title) } else { None },
                    format: Some("markdown".to_string()),
                    content: Some(page.markdown),
                    mode_used: Some("fast".to_string()),
                    fetched_at: Some(now),
                    error: None,
                })
            }
            Err(e) => Ok(ExtractedContent {
                title: None,
                format: Some("markdown".to_string()),
                content: None,
                mode_used: Some("fast".to_string()),
                fetched_at: Some(now),
                error: Some(e.to_string()),
            }),
        }
    }
}
