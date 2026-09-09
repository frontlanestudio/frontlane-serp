use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use url::Url;

use crate::core::error::Result;
use crate::core::http_client::HttpClient;
use crate::core::network_guard::validate_public_url;
use crate::core::response_builder::normalize_url;
use crate::crawl::robots::RobotsTxt;
use crate::extract::{extract_structured_metadata, Extractor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlOptions {
    pub start_url: String,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default = "default_true")]
    pub respect_robots: bool,
    #[serde(default = "default_true")]
    pub extract_content: bool,
}

fn default_max_depth() -> usize {
    2
}

fn default_max_pages() -> usize {
    10
}

fn default_concurrency() -> usize {
    4
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawledPage {
    pub url: String,
    pub depth: usize,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub json_ld: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta_tags: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    pub start_url: String,
    pub pages_crawled: usize,
    pub took_ms: u64,
    pub pages: Vec<CrawledPage>,
}

#[derive(Clone)]
pub struct Crawler {
    http_client: HttpClient,
    extractor: Arc<Extractor>,
}

impl Crawler {
    pub fn new(http_client: HttpClient, extractor: Arc<Extractor>) -> Self {
        Self {
            http_client,
            extractor,
        }
    }

    pub fn extractor(&self) -> &Arc<Extractor> {
        &self.extractor
    }

    pub fn http_client(&self) -> &HttpClient {
        &self.http_client
    }

    pub async fn crawl(&self, options: CrawlOptions) -> Result<CrawlResult> {
        let start_time = Instant::now();

        // Validate start URL
        let parsed_start =
            Url::parse(&options.start_url).map_err(crate::core::error::SerpError::Url)?;
        let start_host = parsed_start.host_str().unwrap_or_default().to_lowercase();

        let allowed_domains: HashSet<String> = if options.allowed_domains.is_empty() {
            let mut set = HashSet::new();
            if !start_host.is_empty() {
                set.insert(start_host.clone());
            }
            set
        } else {
            options
                .allowed_domains
                .iter()
                .map(|d| d.to_lowercase())
                .collect()
        };

        // Fetch robots.txt if requested
        let robots = if options.respect_robots {
            let origin = format!(
                "{}://{}",
                parsed_start.scheme(),
                parsed_start.host_str().unwrap_or_default()
            );
            let robots_url = format!("{}/robots.txt", origin);
            match self.http_client.fetch(&robots_url, None).await {
                Ok(content) => Some(RobotsTxt::parse(&content)),
                Err(_) => None,
            }
        } else {
            None
        };

        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut pages: Vec<CrawledPage> = Vec::new();

        let norm_start = normalize_url(&options.start_url);
        queue.push_back((norm_start.clone(), 0));
        visited.insert(norm_start);

        let max_pages = options.max_pages.clamp(1, 100);
        let max_depth = options.max_depth.min(5);

        let concurrency = options.concurrency.clamp(1, 10);

        while !queue.is_empty() && pages.len() < max_pages {
            let batch_size = concurrency.min(max_pages.saturating_sub(pages.len()));
            let mut batch = Vec::new();
            for _ in 0..batch_size {
                if let Some(item) = queue.pop_front() {
                    batch.push(item);
                } else {
                    break;
                }
            }

            if batch.is_empty() {
                break;
            }

            let mut join_set = tokio::task::JoinSet::new();
            for (current_url, depth) in batch {
                let client = self.http_client.clone();
                let robots_clone = robots.clone();
                let allowed_domains_clone = allowed_domains.clone();
                let extract_content = options.extract_content;

                join_set.spawn(async move {
                    // SSRF guard
                    if validate_public_url(&current_url).await.is_err() {
                        return (current_url, depth, None, Vec::new());
                    }

                    // Robots check
                    if let Some(ref r) = robots_clone {
                        if let Ok(u) = Url::parse(&current_url) {
                            if !r.is_allowed("frontlane-serp", u.path()) {
                                return (current_url, depth, None, Vec::new());
                            }
                        }
                    }

                    // Fetch page
                    let (status, body) = match client
                        .fetch_raw_response(&current_url, None, None, None, None)
                        .await
                    {
                        Ok((st, b)) => (st.as_u16(), b),
                        Err(e) => {
                            let page = CrawledPage {
                                url: current_url.clone(),
                                depth,
                                status: 0,
                                title: None,
                                content: None,
                                json_ld: vec![],
                                meta_tags: HashMap::new(),
                                links: vec![],
                                error: Some(e.to_string()),
                            };
                            return (current_url, depth, Some(page), Vec::new());
                        }
                    };

                    let (json_ld, meta_tags) = extract_structured_metadata(&body);
                    let (title, content, discovered_links) = if extract_content {
                        let page_data = crate::extract::readability::extract_page_content(&body);
                        let links = extract_links(&current_url, &body, &allowed_domains_clone);
                        (
                            if !page_data.title.is_empty() {
                                Some(page_data.title)
                            } else {
                                None
                            },
                            Some(page_data.markdown),
                            links,
                        )
                    } else {
                        let links = extract_links(&current_url, &body, &allowed_domains_clone);
                        (None, None, links)
                    };

                    let page = CrawledPage {
                        url: current_url.clone(),
                        depth,
                        status,
                        title,
                        content,
                        json_ld,
                        meta_tags,
                        links: discovered_links.clone(),
                        error: None,
                    };

                    (current_url, depth, Some(page), discovered_links)
                });
            }

            while let Some(res) = join_set.join_next().await {
                if let Ok((_url, depth, maybe_page, discovered_links)) = res {
                    if let Some(page) = maybe_page {
                        pages.push(page);
                    }
                    if depth < max_depth {
                        for link in discovered_links {
                            let norm = normalize_url(&link);
                            if !norm.is_empty() && visited.insert(norm.clone()) {
                                queue.push_back((norm, depth + 1));
                            }
                        }
                    }
                }
            }
        }

        let took_ms = start_time.elapsed().as_millis() as u64;

        Ok(CrawlResult {
            start_url: options.start_url,
            pages_crawled: pages.len(),
            took_ms,
            pages,
        })
    }
}

pub fn extract_links(base_url: &str, html: &str, allowed_domains: &HashSet<String>) -> Vec<String> {
    let Ok(base) = Url::parse(base_url) else {
        return vec![];
    };

    let document = scraper::Html::parse_document(html);
    let mut links = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(sel) = scraper::Selector::parse("a[href]") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let href = href.trim();
                if href.is_empty()
                    || href.starts_with('#')
                    || href.starts_with("javascript:")
                    || href.starts_with("mailto:")
                    || href.starts_with("tel:")
                {
                    continue;
                }

                if let Ok(joined) = base.join(href) {
                    if joined.scheme() == "http" || joined.scheme() == "https" {
                        if let Some(host) = joined.host_str() {
                            let host_lower = host.to_lowercase();
                            let is_allowed = allowed_domains.is_empty()
                                || allowed_domains.contains(&host_lower)
                                || allowed_domains
                                    .iter()
                                    .any(|d| host_lower.ends_with(&format!(".{}", d)));

                            if is_allowed {
                                let mut clean_url = joined.clone();
                                clean_url.set_fragment(None);
                                let clean_str = clean_url.to_string();
                                if seen.insert(clean_str.clone()) {
                                    links.push(clean_str);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links_domain_filter() {
        let base = "https://example.com/blog";
        let html = r##"
            <html>
            <body>
                <a href="/about">About</a>
                <a href="https://example.com/pricing">Pricing</a>
                <a href="https://sub.example.com/docs">Docs</a>
                <a href="https://other.com/spam">External</a>
                <a href="#section">Anchor</a>
                <a href="javascript:void(0)">JS</a>
            </body>
            </html>
        "##;

        let mut allowed = HashSet::new();
        allowed.insert("example.com".to_string());

        let links = extract_links(base, html, &allowed);
        assert_eq!(links.len(), 3);
        assert!(links.contains(&"https://example.com/about".to_string()));
        assert!(links.contains(&"https://example.com/pricing".to_string()));
        assert!(links.contains(&"https://sub.example.com/docs".to_string()));
        assert!(!links.iter().any(|l| l.contains("other.com")));
    }
}
