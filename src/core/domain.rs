use crate::core::types::{Classification, DomainInfo};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const DEFAULT_ENRICHMENT_DOMAINS_YAML: &str = include_str!("enrichment_domains.yaml");

#[derive(Debug, Deserialize)]
struct EnrichmentFile {
    #[serde(default)]
    domain_source_hints: HashMap<String, String>,
    #[serde(default)]
    news_domains: Vec<String>,
    #[serde(default)]
    forum_domains: Vec<String>,
    #[serde(default)]
    marketplace_domains: Vec<String>,
    #[serde(default)]
    social_domains: Vec<String>,
}

#[derive(Debug, Default)]
struct EnrichmentConfig {
    domain_source_hints: HashMap<String, String>,
    news_domains: HashSet<String>,
    forum_domains: HashSet<String>,
    marketplace_domains: HashSet<String>,
    social_domains: HashSet<String>,
}

static ENRICHMENT_CONFIG: OnceLock<EnrichmentConfig> = OnceLock::new();

fn get_enrichment_config() -> &'static EnrichmentConfig {
    ENRICHMENT_CONFIG.get_or_init(|| {
        let yaml_str = if let Ok(path) = std::env::var("OPENSERP_ENRICHMENT_DOMAINS_FILE") {
            std::fs::read_to_string(path.trim())
                .unwrap_or_else(|_| DEFAULT_ENRICHMENT_DOMAINS_YAML.to_string())
        } else {
            DEFAULT_ENRICHMENT_DOMAINS_YAML.to_string()
        };

        let file: EnrichmentFile = serde_yaml::from_str(&yaml_str).unwrap_or(EnrichmentFile {
            domain_source_hints: HashMap::new(),
            news_domains: Vec::new(),
            forum_domains: Vec::new(),
            marketplace_domains: Vec::new(),
            social_domains: Vec::new(),
        });

        let mut hints = HashMap::new();
        for (d, h) in file.domain_source_hints {
            let nd = normalize_domain(&d);
            if !nd.is_empty() && !h.trim().is_empty() {
                hints.insert(nd, h.trim().to_string());
            }
        }

        let to_set = |vec: Vec<String>| {
            vec.into_iter()
                .map(|d| normalize_domain(&d))
                .filter(|d| !d.is_empty())
                .collect::<HashSet<_>>()
        };

        EnrichmentConfig {
            domain_source_hints: hints,
            news_domains: to_set(file.news_domains),
            forum_domains: to_set(file.forum_domains),
            marketplace_domains: to_set(file.marketplace_domains),
            social_domains: to_set(file.social_domains),
        }
    })
}

pub fn normalize_domain(domain: &str) -> String {
    let mut d = domain.trim().to_lowercase();
    if d.starts_with("www.") {
        d = d[4..].to_string();
    }
    if d.ends_with('.') {
        d.pop();
    }
    d
}

pub fn split_domain(domain: &str) -> (Option<String>, Option<String>) {
    let norm = normalize_domain(domain);
    if norm.is_empty() {
        return (None, None);
    }
    let parts: Vec<&str> = norm.split('.').collect();
    if parts.len() < 2 {
        return (Some(norm), None);
    }

    // Check multi-part tlds like .co.uk, .com.au, .ac.uk, etc.
    let len = parts.len();
    if len >= 3 {
        let last_two = format!("{}.{}", parts[len - 2], parts[len - 1]);
        if matches!(
            last_two.as_str(),
            "co.uk" | "org.uk" | "ac.uk" | "gov.uk" | "com.au" | "net.au" | "co.jp" | "com.br"
        ) {
            let sld = parts[len - 3].to_string();
            return (Some(last_two), Some(sld));
        }
    }

    let tld = parts[len - 1].to_string();
    let sld = parts[len - 2].to_string();
    (Some(tld), Some(sld))
}

fn domain_category(domain: &str, tld: Option<&str>, cfg: &EnrichmentConfig) -> String {
    let tld_str = tld.unwrap_or("");
    if tld_str == "gov" || tld_str.ends_with(".gov") || domain.ends_with(".gov") {
        return "gov".to_string();
    }
    if tld_str == "edu"
        || tld_str.ends_with(".edu")
        || tld_str == "ac.uk"
        || domain.ends_with(".edu")
        || domain.ends_with(".ac.uk")
    {
        return "edu".to_string();
    }
    if tld_str == "mil" {
        return "mil".to_string();
    }
    if cfg.news_domains.contains(domain) {
        return "news".to_string();
    }
    if cfg.forum_domains.contains(domain) {
        return "forum".to_string();
    }
    if cfg.marketplace_domains.contains(domain) {
        return "marketplace".to_string();
    }
    if cfg.social_domains.contains(domain) {
        return "social".to_string();
    }
    String::new()
}

pub fn enrich_domain_info(domain: &str) -> Option<DomainInfo> {
    if domain.is_empty() {
        return None;
    }
    let norm = normalize_domain(domain);
    let (tld, sld) = split_domain(&norm);
    let cfg = get_enrichment_config();
    let category = domain_category(&norm, tld.as_deref(), cfg);

    Some(DomainInfo { tld, sld, category })
}

pub fn classify_content_type(raw_url: &str) -> String {
    let lower = raw_url.to_lowercase();
    if lower.contains("/wiki/") {
        "article".to_string()
    } else if lower.ends_with(".pdf") || lower.contains(".pdf?") {
        "document".to_string()
    } else if lower.contains("/watch?v=") || lower.contains("/video/") || lower.contains("/videos/")
    {
        "video".to_string()
    } else if lower.contains("/forum/")
        || lower.contains("/thread/")
        || lower.contains("/discussion/")
        || lower.contains("/t/")
        || lower.contains("/questions/")
        || lower.contains("/q/")
    {
        "forum_thread".to_string()
    } else if lower.contains("/blog/")
        || lower.contains("/post/")
        || lower.contains("/article/")
        || lower.contains("/news/")
    {
        "article".to_string()
    } else {
        "webpage".to_string()
    }
}

pub fn classify_source_hint(domain: &str) -> Option<String> {
    let cfg = get_enrichment_config();
    let norm = normalize_domain(domain);
    if let Some(hint) = cfg.domain_source_hints.get(&norm) {
        return Some(hint.clone());
    }
    let (tld, sld) = split_domain(&norm);
    if let (Some(t), Some(s)) = (tld, sld) {
        let registrable = format!("{}.{}", s, t);
        if let Some(hint) = cfg.domain_source_hints.get(&registrable) {
            return Some(hint.clone());
        }
    }
    None
}

pub fn classify_url(raw_url: &str, domain: &str) -> Option<Classification> {
    if raw_url.is_empty() && domain.is_empty() {
        return None;
    }
    let content_type = classify_content_type(raw_url);
    let source_hint = classify_source_hint(domain);
    if content_type == "webpage" && source_hint.is_none() {
        return None;
    }
    Some(Classification {
        content_type: Some(content_type),
        source_hint,
    })
}
