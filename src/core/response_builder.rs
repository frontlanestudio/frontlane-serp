use base64::Engine;
use chrono::{DateTime, Utc};
use md5::{Digest, Md5};
use std::collections::HashSet;
use url::Url;

use crate::core::domain::{classify_url, enrich_domain_info, normalize_domain};
use crate::core::types::{
    ImageData, ImageResult, ImageSource, Position, ResultItem, ResultType, SearchResult,
    SerpFeature,
};

pub const RESPONSE_ID_BYTES: usize = 8;

pub fn short_md5(value: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(value.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..RESPONSE_ID_BYTES])
}

pub fn build_result_id(engine: &str, normalized_url: &str) -> String {
    format!("s_{}", short_md5(&format!("{}|{}", engine, normalized_url)))
}

pub fn build_image_id(engine: &str, image_url: &str) -> String {
    format!("i_{}", short_md5(&format!("{}|{}", engine, image_url)))
}

pub fn build_cluster_id(normalized_url: &str) -> String {
    format!("c_{}", short_md5(normalized_url))
}

pub fn build_feature_id(feature: &SerpFeature) -> String {
    let primary_link = if let Some(first) = feature.links.first() {
        first.url.clone().unwrap_or_default()
    } else if let Some(first) = feature.items.first() {
        first.link.clone().unwrap_or_default()
    } else {
        String::new()
    };

    let key = format!(
        "{}|{:?}|{}|{}|{}",
        feature.engine,
        feature.feature_type,
        feature.title.as_deref().unwrap_or("").trim().to_lowercase(),
        feature.text.as_deref().unwrap_or("").trim().to_lowercase(),
        primary_link
    );
    format!("f_{}", short_md5(&key))
}

pub fn normalize_url(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }

    let raw = if raw.contains("bing.com/ck/a") {
        unwrap_bing_url(raw).unwrap_or_else(|| raw.to_string())
    } else {
        raw.to_string()
    };

    let Ok(mut u) = Url::parse(&raw) else {
        return raw;
    };

    let _ = u.set_scheme(&u.scheme().to_lowercase());
    if let Some(host) = u.host_str() {
        let _ = u.set_host(Some(&host.to_lowercase()));
    }

    let tracking_params: HashSet<&str> = [
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
        "msclkid",
        "ref",
        "_ga",
    ]
    .into_iter()
    .collect();

    let filtered_pairs: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| !tracking_params.contains(k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if filtered_pairs.is_empty() {
        u.set_query(None);
    } else {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in filtered_pairs {
            serializer.append_pair(&k, &v);
        }
        u.set_query(Some(&serializer.finish()));
    }

    let mut normalized = u.to_string();
    if u.path() != "/" && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

pub fn unwrap_bing_url(raw: &str) -> Option<String> {
    let u = Url::parse(raw).ok()?;
    let u_param = u.query_pairs().find(|(k, _)| k == "u")?.1.into_owned();
    if !u_param.starts_with("a1") {
        return None;
    }
    let encoded = &u_param[2..];
    let mut padded = encoded.to_string();
    match padded.len() % 4 {
        2 => padded.push_str("=="),
        3 => padded.push('='),
        _ => {}
    }
    let standard = padded.replace('-', "+").replace('_', "/");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(standard)
        .ok()?;
    let candidate = String::from_utf8(decoded).ok()?;
    if candidate.starts_with("http://") || candidate.starts_with("https://") {
        Some(candidate)
    } else {
        None
    }
}

pub fn extract_domain(raw_url: &str) -> String {
    if let Ok(u) = Url::parse(raw_url) {
        if let Some(host) = u.host_str() {
            return normalize_domain(host);
        }
    }
    String::new()
}

pub fn build_display_url(raw_url: &str, domain: &str) -> String {
    let Ok(u) = Url::parse(raw_url) else {
        return domain.to_string();
    };
    let path = u.path().trim_matches('/');
    if path.is_empty() {
        return domain.to_string();
    }
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return domain.to_string();
    }
    let mut breadcrumb = format!("{} › {}", domain, segments.join(" › "));
    if breadcrumb.chars().count() > 60 {
        let mut truncated: String = breadcrumb.chars().take(57).collect();
        truncated.push_str("...");
        breadcrumb = truncated;
    }
    breadcrumb
}

pub fn compute_result_position(raw: &SearchResult, start: usize) -> usize {
    if raw.absolute_rank > 0 {
        return raw.absolute_rank as usize;
    }
    let rank = if raw.rank < 0 {
        (-raw.rank) as usize
    } else {
        raw.rank as usize
    };
    if rank == 0 {
        return 0;
    }
    if start > 0 && rank > start {
        return rank;
    }
    start + rank
}

pub fn enrich_result(raw: SearchResult, engine: &str, start: usize) -> ResultItem {
    let normalized_url = normalize_url(&raw.url);
    let domain = extract_domain(&normalized_url);
    let display_url = build_display_url(&normalized_url, &domain);
    let favicon = if !domain.is_empty() {
        format!("https://{}/favicon.ico", domain)
    } else {
        String::new()
    };

    let mut result_type = raw.result_type;
    if raw.ad {
        result_type = ResultType::Ad;
    } else if raw.rank <= 0 && result_type == ResultType::Organic {
        result_type = ResultType::AnswerBox;
    }

    let rank = if raw.rank < 0 {
        if raw.ad {
            (-raw.rank) as usize
        } else {
            0
        }
    } else {
        raw.rank as usize
    };

    let mut absolute = compute_result_position(&raw, start);
    if absolute == 0 {
        absolute = rank;
    }

    let position = if absolute > 0 {
        Some(Position { absolute })
    } else {
        None
    };

    let domain_info = enrich_domain_info(&domain);
    let classification = classify_url(&normalized_url, &domain);

    ResultItem {
        id: build_result_id(engine, &normalized_url),
        rank,
        result_type,
        title: raw.title,
        url: normalized_url,
        display_url,
        snippet: raw.description,
        domain,
        favicon,
        position,
        engine: engine.to_string(),
        domain_info,
        classification,
        extracted: None,
        score: None,
        engine_consensus: None,
        engines: None,
    }
}

pub fn enrich_serp_feature(
    mut raw: SerpFeature,
    engine: &str,
    source_result_id: Option<&str>,
    extracted_at: DateTime<Utc>,
) -> SerpFeature {
    raw.engine = engine.to_string();
    if let Some(src_id) = source_result_id {
        if !raw.source_result_ids.iter().any(|id| id == src_id) {
            raw.source_result_ids.push(src_id.to_string());
        }
    }
    if raw.id.is_empty() {
        raw.id = build_feature_id(&raw);
    }
    if raw.extracted_at.is_empty() {
        raw.extracted_at = extracted_at.to_rfc3339();
    }
    raw
}

pub fn enrich_image_result(raw: SearchResult, engine: &str) -> ImageResult {
    let image_url = normalize_url(&raw.url);
    let (thumbnail, width, height) = if let Some(ref img) = raw.image_data {
        (img.thumbnail.clone(), img.width, img.height)
    } else {
        (None, None, None)
    };

    let (page_url, domain) = if let Some(ref src) = raw.image_source {
        let p_url = normalize_url(&src.page_url);
        let dom = if src.domain.is_empty() {
            extract_domain(&p_url)
        } else {
            src.domain.clone()
        };
        (p_url, dom)
    } else {
        (image_url.clone(), extract_domain(&image_url))
    };

    ImageResult {
        id: build_image_id(engine, &image_url),
        rank: if raw.rank < 0 { 0 } else { raw.rank as usize },
        result_type: ResultType::Image,
        title: raw.title,
        image: ImageData {
            url: image_url,
            thumbnail,
            width,
            height,
        },
        source: ImageSource { page_url, domain },
        engine: engine.to_string(),
    }
}
