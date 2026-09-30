use crate::core::http_client::HttpClient;
use serde::{Deserialize, Serialize};
use url::Url;

pub const LLMS_TXT_CANDIDATES: &[&str] = &["/llms-full.txt", "/llms.txt"];
pub const MIN_LLMS_TXT_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmsTxtLink {
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmsTxtSection {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<LlmsTxtLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LlmsTxtDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<LlmsTxtSection>,
}

pub fn is_site_root(raw_url: &str) -> bool {
    if let Ok(u) = Url::parse(raw_url) {
        let path = u.path().trim_matches('/');
        return path.is_empty();
    }
    false
}

fn parse_markdown_link(line: &str) -> Option<LlmsTxtLink> {
    let trimmed = line.trim();
    let rest = if let Some(stripped) = trimmed.strip_prefix('-') {
        stripped.trim()
    } else if let Some(stripped) = trimmed.strip_prefix('*') {
        stripped.trim()
    } else {
        return None;
    };

    let open_bracket = rest.find('[')?;
    let close_bracket = rest[open_bracket..].find("](")? + open_bracket;
    let title = &rest[open_bracket + 1..close_bracket];

    let url_start = close_bracket + 2;
    let close_paren = rest[url_start..].find(')')? + url_start;
    let url = &rest[url_start..close_paren];

    let after_paren = rest[close_paren + 1..].trim();
    let summary = if after_paren.is_empty() {
        None
    } else {
        let cleaned = after_paren
            .strip_prefix(':')
            .or_else(|| after_paren.strip_prefix('-'))
            .unwrap_or(after_paren)
            .trim();
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned.to_string())
        }
    };

    Some(LlmsTxtLink {
        title: title.trim().to_string(),
        url: url.trim().to_string(),
        summary,
    })
}

pub fn parse_llms_txt(content: &str) -> LlmsTxtDocument {
    let mut doc = LlmsTxtDocument::default();
    let mut current_section: Option<LlmsTxtSection> = None;
    let mut summary_lines: Vec<String> = Vec::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // H1 Title: # Title
        if line.starts_with("# ") && !line.starts_with("## ") {
            if doc.title.is_none() {
                doc.title = Some(line.trim_start_matches('#').trim().to_string());
                continue;
            }
        }

        // Section header: ## Section or ### Section
        if line.starts_with("## ") || line.starts_with("### ") {
            if let Some(sec) = current_section.take() {
                doc.sections.push(sec);
            }
            let sec_title = line.trim_start_matches('#').trim().to_string();
            current_section = Some(LlmsTxtSection {
                title: sec_title,
                links: Vec::new(),
            });
            continue;
        }

        // Blockquote / summary line (before any section)
        if current_section.is_none() && line.starts_with('>') {
            let quote_text = line.trim_start_matches('>').trim();
            if !quote_text.is_empty() {
                summary_lines.push(quote_text.to_string());
            }
            continue;
        }

        // List item link
        if let Some(link) = parse_markdown_link(line) {
            if let Some(sec) = &mut current_section {
                sec.links.push(link);
            } else {
                let sec = LlmsTxtSection {
                    title: String::new(),
                    links: vec![link],
                };
                current_section = Some(sec);
            }
            continue;
        }

        // Plain text summary before sections if we don't have blockquotes
        if current_section.is_none() && doc.title.is_some() && summary_lines.is_empty() {
            summary_lines.push(line.to_string());
        }
    }

    if let Some(sec) = current_section {
        doc.sections.push(sec);
    }

    if !summary_lines.is_empty() {
        doc.summary = Some(summary_lines.join(" "));
    }

    doc
}

pub async fn try_llms_txt(client: &HttpClient, raw_url: &str) -> Option<(String, String)> {
    if !is_site_root(raw_url) {
        return None;
    }
    let base = Url::parse(raw_url).ok()?;

    for candidate in LLMS_TXT_CANDIDATES {
        if let Ok(target_url) = base.join(candidate) {
            if let Ok(content) = client.fetch(target_url.as_str(), None).await {
                let trimmed = content.trim();
                if trimmed.len() >= MIN_LLMS_TXT_CHARS
                    && !trimmed.starts_with("<!DOCTYPE")
                    && !trimmed.starts_with("<html")
                    && !trimmed.starts_with("<body")
                {
                    return Some((target_url.to_string(), trimmed.to_string()));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_site_root() {
        assert!(is_site_root("https://example.com"));
        assert!(is_site_root("https://example.com/"));
        assert!(is_site_root("http://localhost:8080/"));
        assert!(is_site_root("https://sub.domain.org/"));

        assert!(!is_site_root("https://example.com/blog"));
        assert!(!is_site_root("https://example.com/blog/"));
        assert!(!is_site_root("https://example.com/docs/api.html"));
        assert!(!is_site_root("not-a-valid-url"));
        assert!(!is_site_root(""));
    }

    #[test]
    fn test_parse_llms_txt_standard_format() {
        let sample = r#"# Frontlane SERP
> Fast, open-source search engine scraper and retrieval tool for LLMs.

A lightweight Rust-based SERP API providing structured results without API keys.

## Core Documentation
- [Getting Started](https://frontlane.dev/docs/quickstart): Quickstart guide for new developers
- [API Reference](https://frontlane.dev/docs/api): Full OpenAPI specification
- [Engines](https://frontlane.dev/docs/engines)

## Optional Resources
- [GitHub Repository](https://github.com/frontlanestudio/frontlane-serp) - Complete source code
* [Crates.io](https://crates.io/crates/frontlane-serp): Published Rust crate
"#;

        let doc = parse_llms_txt(sample);

        assert_eq!(doc.title.as_deref(), Some("Frontlane SERP"));
        assert_eq!(
            doc.summary.as_deref(),
            Some("Fast, open-source search engine scraper and retrieval tool for LLMs.")
        );
        assert_eq!(doc.sections.len(), 2);

        // First Section
        let sec1 = &doc.sections[0];
        assert_eq!(sec1.title, "Core Documentation");
        assert_eq!(sec1.links.len(), 3);
        assert_eq!(sec1.links[0].title, "Getting Started");
        assert_eq!(sec1.links[0].url, "https://frontlane.dev/docs/quickstart");
        assert_eq!(
            sec1.links[0].summary.as_deref(),
            Some("Quickstart guide for new developers")
        );

        assert_eq!(sec1.links[1].title, "API Reference");
        assert_eq!(sec1.links[1].url, "https://frontlane.dev/docs/api");
        assert_eq!(
            sec1.links[1].summary.as_deref(),
            Some("Full OpenAPI specification")
        );

        assert_eq!(sec1.links[2].title, "Engines");
        assert_eq!(sec1.links[2].url, "https://frontlane.dev/docs/engines");
        assert_eq!(sec1.links[2].summary, None);

        // Second Section
        let sec2 = &doc.sections[1];
        assert_eq!(sec2.title, "Optional Resources");
        assert_eq!(sec2.links.len(), 2);
        assert_eq!(sec2.links[0].title, "GitHub Repository");
        assert_eq!(
            sec2.links[0].url,
            "https://github.com/frontlanestudio/frontlane-serp"
        );
        assert_eq!(
            sec2.links[0].summary.as_deref(),
            Some("Complete source code")
        );

        assert_eq!(sec2.links[1].title, "Crates.io");
        assert_eq!(
            sec2.links[1].url,
            "https://crates.io/crates/frontlane-serp"
        );
        assert_eq!(
            sec2.links[1].summary.as_deref(),
            Some("Published Rust crate")
        );
    }

    #[test]
    fn test_parse_llms_txt_empty_and_minimal() {
        let empty_doc = parse_llms_txt("");
        assert_eq!(empty_doc.title, None);
        assert_eq!(empty_doc.summary, None);
        assert!(empty_doc.sections.is_empty());

        let whitespace_doc = parse_llms_txt("   \n\n\t  \n");
        assert_eq!(whitespace_doc.title, None);
        assert!(whitespace_doc.sections.is_empty());

        let title_only = parse_llms_txt("# Only A Title\n");
        assert_eq!(title_only.title.as_deref(), Some("Only A Title"));
        assert_eq!(title_only.summary, None);
        assert!(title_only.sections.is_empty());
    }

    #[test]
    fn test_parse_llms_txt_link_variations() {
        let markdown = r#"# Link Variations
- [Title With Spaces](https://example.com/path?query=val&num=1): Standard colon summary
* [Asterisk Bullet](https://example.com/doc) - Hyphen summary
- [No Summary Link](https://example.com/simple)
- [Trailing Whitespace](https://example.com/ws) :   Summary with whitespace   
- [Link With Colons: In: Title](https://example.com/colons): Summary with: colons
"#;
        let doc = parse_llms_txt(markdown);
        assert_eq!(doc.title.as_deref(), Some("Link Variations"));
        assert_eq!(doc.sections.len(), 1);
        let links = &doc.sections[0].links;
        assert_eq!(links.len(), 5);

        assert_eq!(links[0].title, "Title With Spaces");
        assert_eq!(
            links[0].url,
            "https://example.com/path?query=val&num=1"
        );
        assert_eq!(
            links[0].summary.as_deref(),
            Some("Standard colon summary")
        );

        assert_eq!(links[1].title, "Asterisk Bullet");
        assert_eq!(links[1].url, "https://example.com/doc");
        assert_eq!(links[1].summary.as_deref(), Some("Hyphen summary"));

        assert_eq!(links[2].title, "No Summary Link");
        assert_eq!(links[2].url, "https://example.com/simple");
        assert_eq!(links[2].summary, None);

        assert_eq!(links[3].title, "Trailing Whitespace");
        assert_eq!(
            links[3].summary.as_deref(),
            Some("Summary with whitespace")
        );

        assert_eq!(links[4].title, "Link With Colons: In: Title");
        assert_eq!(
            links[4].summary.as_deref(),
            Some("Summary with: colons")
        );
    }

    #[test]
    fn test_llms_txt_serialization() {
        let doc = LlmsTxtDocument {
            title: Some("Sample Doc".to_string()),
            summary: Some("A summary for testing".to_string()),
            sections: vec![LlmsTxtSection {
                title: "Docs".to_string(),
                links: vec![LlmsTxtLink {
                    title: "Home".to_string(),
                    url: "https://example.com".to_string(),
                    summary: Some("Main site".to_string()),
                }],
            }],
        };

        let json = serde_json::to_string(&doc).expect("Serialization should succeed");
        assert!(json.contains("\"title\":\"Sample Doc\""));
        assert!(json.contains("\"summary\":\"A summary for testing\""));
        assert!(json.contains("\"url\":\"https://example.com\""));

        let deserialized: LlmsTxtDocument =
            serde_json::from_str(&json).expect("Deserialization should succeed");
        assert_eq!(deserialized, doc);
    }

    #[test]
    fn test_llms_txt_constants() {
        assert_eq!(LLMS_TXT_CANDIDATES, &["/llms-full.txt", "/llms.txt"]);
        assert_eq!(MIN_LLMS_TXT_CHARS, 200);
    }
}
