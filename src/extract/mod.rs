pub mod llmstxt;
pub mod readability;

use crate::core::captcha::CaptchaSolver;
use crate::core::error::Result;
use crate::core::http_client::HttpClient;
use crate::core::proxy::{LaneStore, ProxyLaneKey};
use crate::core::types::ExtractedContent;
use chrono::Utc;

/// Detects whether an HTTP response is a Cloudflare JavaScript / Turnstile / Managed Challenge page.
pub fn is_cloudflare_challenge(status: reqwest::StatusCode, body: &str) -> bool {
    // Cloudflare challenges typically return 403 Forbidden or 503 Service Unavailable,
    // or sometimes 200 with an interstitial challenge template.
    let status_code = status.as_u16();
    let is_challenge_status = status_code == 403 || status_code == 503 || status_code == 429;

    let body_lower = body.to_lowercase();
    let has_challenge_marker = body_lower.contains("attention required! | cloudflare")
        || body_lower.contains("cf-wrapper")
        || body_lower.contains("cf-chl-widget")
        || body_lower.contains("cf-turnstile")
        || body_lower.contains("checking your browser")
        || body_lower.contains("just a moment...")
        || body_lower.contains("cf-challenge-running")
        || body_lower.contains("cf-alert")
        || body_lower.contains("please enable cookies");

    is_challenge_status && has_challenge_marker
        || (has_challenge_marker && body_lower.contains("cloudflare"))
}

#[derive(Clone)]
pub struct Extractor {
    http_client: HttpClient,
    solver: Option<CaptchaSolver>,
    lane_store: Option<LaneStore>,
}

impl Extractor {
    pub fn new(http_client: HttpClient) -> Self {
        Self {
            http_client,
            solver: None,
            lane_store: None,
        }
    }

    pub fn with_solver_and_lanes(
        http_client: HttpClient,
        solver: Option<CaptchaSolver>,
        lane_store: Option<LaneStore>,
    ) -> Self {
        Self {
            http_client,
            solver,
            lane_store,
        }
    }

    pub async fn extract(&self, url: &str, use_llmstxt: bool) -> Result<ExtractedContent> {
        self.extract_with_options(url, use_llmstxt, None, None)
            .await
    }

    pub async fn extract_with_options(
        &self,
        url: &str,
        use_llmstxt: bool,
        proxy_url: Option<&str>,
        lane_key: Option<&ProxyLaneKey>,
    ) -> Result<ExtractedContent> {
        let now = Utc::now().to_rfc3339();

        // SSRF guard: validate that target URL is a public endpoint
        if let Err(e) = crate::core::network_guard::validate_public_url(url).await {
            return Ok(ExtractedContent {
                title: None,
                format: Some("markdown".to_string()),
                content: None,
                mode_used: Some("blocked".to_string()),
                fetched_at: Some(now),
                error: Some(e.to_string()),
                json_ld: Vec::new(),
                meta_tags: std::collections::HashMap::new(),
            });
        }

        if use_llmstxt {
            if let Some((_, text)) = llmstxt::try_llms_txt(&self.http_client, url).await {
                return Ok(ExtractedContent {
                    title: Some("llms.txt".to_string()),
                    format: Some("markdown".to_string()),
                    content: Some(text),
                    mode_used: Some("llmstxt".to_string()),
                    fetched_at: Some(now),
                    error: None,
                    json_ld: Vec::new(),
                    meta_tags: std::collections::HashMap::new(),
                });
            }
        }

        // 1. Check if we already have a cached cf_clearance in the lane
        let domain = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_default();

        let mut existing_clearance = None;
        if let (Some(ref lanes), Some(key)) = (&self.lane_store, lane_key) {
            if !domain.is_empty() {
                existing_clearance = lanes.get_clearance(key, &domain).await;
            }
        }

        let initial_cookie = existing_clearance
            .as_ref()
            .map(|c| format!("cf_clearance={}", c.cf_clearance));
        let initial_ua = existing_clearance.as_ref().map(|c| c.user_agent.as_str());

        // 2. Fetch raw page
        let (status, body) = match self
            .http_client
            .fetch_raw_response(url, None, initial_ua, initial_cookie.as_deref(), None)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                return Ok(ExtractedContent {
                    title: None,
                    format: Some("markdown".to_string()),
                    content: None,
                    mode_used: Some("fast".to_string()),
                    fetched_at: Some(now),
                    error: Some(e.to_string()),
                    json_ld: Vec::new(),
                    meta_tags: std::collections::HashMap::new(),
                });
            }
        };

        // 3. Check for Cloudflare Challenge
        if is_cloudflare_challenge(status, &body) {
            // Check if challenge solver is enabled
            if let Some(ref solver) = self.solver {
                if solver.is_enabled() {
                    let ua = self.http_client.user_agent();
                    match solver.solve_cloudflare_challenge(url, proxy_url, ua).await {
                        Ok(clearance) => {
                            // Save to lane store if available
                            if let (Some(ref lanes), Some(key)) = (&self.lane_store, lane_key) {
                                lanes.set_clearance(key, clearance.clone()).await;
                            }

                            // Re-request with obtained cf_clearance and matching user_agent
                            let cookie_header = format!("cf_clearance={}", clearance.cf_clearance);
                            match self
                                .http_client
                                .fetch_raw_response(
                                    url,
                                    None,
                                    Some(&clearance.user_agent),
                                    Some(&cookie_header),
                                    None,
                                )
                                .await
                            {
                                Ok((bypassed_status, bypassed_body)) => {
                                    if !is_cloudflare_challenge(bypassed_status, &bypassed_body)
                                        && (bypassed_status.is_success()
                                            || bypassed_status.as_u16() == 200)
                                    {
                                        let page =
                                            readability::extract_page_content(&bypassed_body);
                                        let (json_ld, meta_tags) =
                                            extract_structured_metadata(&bypassed_body);
                                        return Ok(ExtractedContent {
                                            title: if !page.title.is_empty() {
                                                Some(page.title)
                                            } else {
                                                None
                                            },
                                            format: Some("markdown".to_string()),
                                            content: Some(page.markdown),
                                            mode_used: Some("fast+cf_clearance".to_string()),
                                            fetched_at: Some(now),
                                            error: None,
                                            json_ld,
                                            meta_tags,
                                        });
                                    }
                                }
                                Err(e) => {
                                    return Ok(ExtractedContent {
                                        title: None,
                                        format: Some("markdown".to_string()),
                                        content: None,
                                        mode_used: Some("fast+cf_clearance".to_string()),
                                        fetched_at: Some(now),
                                        error: Some(format!(
                                            "failed to fetch after challenge solve: {}",
                                            e
                                        )),
                                        json_ld: Vec::new(),
                                        meta_tags: std::collections::HashMap::new(),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            return Ok(ExtractedContent {
                                title: None,
                                format: Some("markdown".to_string()),
                                content: None,
                                mode_used: Some("fast".to_string()),
                                fetched_at: Some(now),
                                error: Some(format!("cloudflare challenge solve failed: {}", e)),
                                json_ld: Vec::new(),
                                meta_tags: std::collections::HashMap::new(),
                            });
                        }
                    }
                }
            }

            // Cloudflare blocked and solver was not enabled or did not bypass
            return Ok(ExtractedContent {
                title: None,
                format: Some("markdown".to_string()),
                content: None,
                mode_used: Some("fast".to_string()),
                fetched_at: Some(now),
                error: Some(format!(
                    "blocked by Cloudflare challenge (HTTP {})",
                    status.as_u16()
                )),
                json_ld: Vec::new(),
                meta_tags: std::collections::HashMap::new(),
            });
        }

        // Check if other HTTP error
        if status.as_u16() == 403 || status.as_u16() == 407 {
            return Ok(ExtractedContent {
                title: None,
                format: Some("markdown".to_string()),
                content: None,
                mode_used: Some("fast".to_string()),
                fetched_at: Some(now),
                error: Some(format!("HTTP status {}", status)),
                json_ld: Vec::new(),
                meta_tags: std::collections::HashMap::new(),
            });
        }

        let page = readability::extract_page_content(&body);
        let (json_ld, meta_tags) = extract_structured_metadata(&body);
        let mode_used = if initial_cookie.is_some() {
            "fast+cached_clearance".to_string()
        } else {
            "fast".to_string()
        };

        Ok(ExtractedContent {
            title: if !page.title.is_empty() {
                Some(page.title)
            } else {
                None
            },
            format: Some("markdown".to_string()),
            content: Some(page.markdown),
            mode_used: Some(mode_used),
            fetched_at: Some(now),
            error: None,
            json_ld,
            meta_tags,
        })
    }
}

pub fn extract_structured_metadata(
    html: &str,
) -> (
    Vec<serde_json::Value>,
    std::collections::HashMap<String, String>,
) {
    let document = scraper::Html::parse_document(html);

    // 1. JSON-LD scripts
    let mut json_ld = Vec::new();
    if let Ok(script_sel) = scraper::Selector::parse(r#"script[type="application/ld+json"]"#) {
        for el in document.select(&script_sel) {
            let raw_text = el.text().collect::<Vec<_>>().join("");
            let trimmed = raw_text.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let serde_json::Value::Array(arr) = val {
                    json_ld.extend(arr);
                } else {
                    json_ld.push(val);
                }
            }
        }
    }

    // 2. OpenGraph, Twitter, and standard meta tags
    let mut meta_tags = std::collections::HashMap::new();
    if let Ok(meta_sel) = scraper::Selector::parse("meta") {
        for el in document.select(&meta_sel) {
            let key = el
                .value()
                .attr("property")
                .or_else(|| el.value().attr("name"))
                .or_else(|| el.value().attr("itemprop"));
            let content = el.value().attr("content");
            if let (Some(k), Some(c)) = (key, content) {
                let k_norm = k.trim().to_lowercase();
                let c_trimmed = c.trim().to_string();
                if !k_norm.is_empty()
                    && !c_trimmed.is_empty()
                    && (k_norm.starts_with("og:")
                        || k_norm.starts_with("twitter:")
                        || k_norm == "description"
                        || k_norm == "keywords"
                        || k_norm == "author"
                        || k_norm == "article:published_time"
                        || k_norm == "article:author")
                {
                    meta_tags.entry(k_norm).or_insert(c_trimmed);
                }
            }
        }
    }

    (json_ld, meta_tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::captcha::{CaptchaSolverConfig, CloudflareClearance};
    use reqwest::StatusCode;

    #[test]
    fn test_detect_cloudflare_challenge() {
        let cf_html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Attention Required! | Cloudflare</title></head>
            <body>
                <div id="cf-wrapper">
                    <h1>Sorry, you have been blocked</h1>
                    <h2>Checking your browser...</h2>
                </div>
            </body>
            </html>
        "#;

        assert!(is_cloudflare_challenge(StatusCode::FORBIDDEN, cf_html));
        assert!(is_cloudflare_challenge(
            StatusCode::SERVICE_UNAVAILABLE,
            cf_html
        ));

        let normal_html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>My Clean Web Page</title></head>
            <body><main><p>Real content here</p></main></body>
            </html>
        "#;
        assert!(!is_cloudflare_challenge(StatusCode::OK, normal_html));
        assert!(!is_cloudflare_challenge(StatusCode::FORBIDDEN, normal_html));
    }

    #[tokio::test]
    async fn test_extractor_with_mock_challenge_solver() {
        let http_client = HttpClient::new(None, true, 10).unwrap();

        let mock_clearance = CloudflareClearance::new(
            "cloudflare-protected.example",
            "valid_cf_clearance_cookie_value",
            http_client.user_agent(),
            1200,
            None,
        );

        let solver =
            CaptchaSolver::new(CaptchaSolverConfig::default()).with_mock_solution(mock_clearance);
        let lane_store = LaneStore::new(10);

        let extractor =
            Extractor::with_solver_and_lanes(http_client, Some(solver), Some(lane_store.clone()));

        let key = ProxyLaneKey::new("default", "extract", "test-session");
        assert_eq!(key.session_id, "test-session");
        // Test that extractor initializes correctly and stores lanes
        assert!(extractor.solver.is_some());
        assert!(extractor.lane_store.is_some());
    }

    #[test]
    fn test_extract_structured_metadata() {
        let sample_html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Rust Web Development</title>
                <meta property="og:title" content="Rust Web Development Guide" />
                <meta property="og:description" content="A comprehensive guide to async web apps in Rust." />
                <meta property="og:type" content="article" />
                <meta name="twitter:card" content="summary_large_image" />
                <meta name="author" content="Brandon Hubbard" />
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@type": "Article",
                    "headline": "Rust Web Development Guide",
                    "author": {
                        "@type": "Person",
                        "name": "Brandon Hubbard"
                    }
                }
                </script>
            </head>
            <body>
                <article><h1>Rust Web Development Guide</h1><p>Content goes here.</p></article>
            </body>
            </html>
        "#;

        let (json_ld, meta_tags) = extract_structured_metadata(sample_html);
        assert_eq!(json_ld.len(), 1);
        assert_eq!(json_ld[0]["@type"], "Article");
        assert_eq!(json_ld[0]["headline"], "Rust Web Development Guide");

        assert_eq!(
            meta_tags.get("og:title").unwrap(),
            "Rust Web Development Guide"
        );
        assert_eq!(
            meta_tags.get("og:description").unwrap(),
            "A comprehensive guide to async web apps in Rust."
        );
        assert_eq!(meta_tags.get("og:type").unwrap(), "article");
        assert_eq!(
            meta_tags.get("twitter:card").unwrap(),
            "summary_large_image"
        );
        assert_eq!(meta_tags.get("author").unwrap(), "Brandon Hubbard");
    }
}
