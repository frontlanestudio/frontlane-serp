use crate::audit::types::PageAuditResult;
use scraper::{Html, Selector};
use std::sync::LazyLock;
use url::Url;

static WORD_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b[A-Za-z0-9'-]+\b").unwrap());

pub fn parse_page_audit(
    html_str: &str,
    page_url: &str,
    target_keyword: &str,
    status_code: u16,
    rank_label: String,
    rank_num: Option<usize>,
    is_target: bool,
) -> PageAuditResult {
    let doc = Html::parse_document(html_str);
    let norm_kw = target_keyword.trim().to_lowercase();
    let kw_slug = norm_kw.replace(' ', "-");

    // Domain
    let domain = Url::parse(page_url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_string())
        })
        .unwrap_or_default();

    // Title
    let title = doc
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
        .unwrap_or_default();
    let title_lower = title.to_lowercase();
    let title_exact_match = title_lower.contains(&norm_kw);
    let title_starts_with_kw = title_lower.starts_with(&norm_kw);

    // Meta description
    let meta_desc = doc
        .select(&Selector::parse("meta[name='description'], meta[name='Description']").unwrap())
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("")
        .trim()
        .to_string();
    let meta_exact_match = meta_desc.to_lowercase().contains(&norm_kw);

    // Headings
    let h1: Vec<String> = doc
        .select(&Selector::parse("h1").unwrap())
        .map(|el| {
            el.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect();
    let h1_exact_match = h1.iter().any(|h| h.to_lowercase().contains(&norm_kw));

    let h2_headings: Vec<String> = doc
        .select(&Selector::parse("h2").unwrap())
        .map(|el| {
            el.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty() && s.len() > 3)
        .collect();
    let h2_count = h2_headings.len();

    let h3_headings: Vec<String> = doc
        .select(&Selector::parse("h3").unwrap())
        .map(|el| {
            el.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty() && s.len() > 3)
        .collect();

    // Questions extracted from H2 & H3
    let mut questions_found = Vec::new();
    let question_starters = [
        "what", "how", "why", "can", "when", "where", "who", "do", "does", "is", "are", "which",
        "should",
    ];
    for heading in h2_headings.iter().chain(h3_headings.iter()) {
        let hl = heading.to_lowercase();
        if (heading.ends_with('?') || question_starters.iter().any(|&q| hl.starts_with(q)))
            && !questions_found.contains(heading)
        {
            questions_found.push(heading.clone());
        }
    }

    // Body content (excluding chrome elements)
    let body_text = doc
        .select(&Selector::parse("body").unwrap())
        .next()
        .map(|body| {
            let mut texts = Vec::new();
            for node in body.descendants() {
                if let Some(el) = node.value().as_element() {
                    let name = el.name();
                    if matches!(
                        name,
                        "script" | "style" | "noscript" | "svg" | "header" | "footer" | "nav"
                    ) {
                        continue;
                    }
                }
                if let Some(text) = node.value().as_text() {
                    texts.push(text.trim());
                }
            }
            texts.join(" ")
        })
        .unwrap_or_default();

    let words: Vec<&str> = WORD_RE.find_iter(&body_text).map(|m| m.as_str()).collect();
    let word_count = words.len();

    let body_lower = body_text.to_lowercase();
    let exact_keyword_count = if !norm_kw.is_empty() {
        if let Ok(re) = regex::Regex::new(&format!(r"\b{}\b", regex::escape(&norm_kw))) {
            re.find_iter(&body_lower).count()
        } else {
            0
        }
    } else {
        0
    };

    let kw_word_len = norm_kw.split_whitespace().count().max(1);
    let keyword_density_pct = if word_count > 0 {
        ((exact_keyword_count * kw_word_len) as f64 / word_count as f64) * 100.0
    } else {
        0.0
    };

    // Slug / Dedicated Page analysis
    let url_lower = page_url.to_lowercase();
    let slug_has_exact_kw = url_lower.contains(&kw_slug);
    // Dynamic query word presence in URL path
    let kw_words: Vec<&str> = norm_kw
        .split_whitespace()
        .filter(|w| w.len() >= 3)
        .collect();
    let path = Url::parse(page_url)
        .map(|u| u.path().to_string())
        .unwrap_or_default();
    let path_lower = path.to_lowercase();
    let slug_has_kw_terms = !kw_words.is_empty()
        && kw_words.iter().filter(|&&w| path_lower.contains(w)).count()
            >= (kw_words.len() / 2).max(1);
    let is_dedicated_page =
        path.len() > 1 && path != "/" && (slug_has_exact_kw || slug_has_kw_terms);
    let slug_has_geo = slug_has_kw_terms;

    // Directory / Aggregator detection
    let is_directory_aggregator = crate::core::domain::is_directory_domain(&domain)
        || domain.contains("wikipedia.org")
        || domain.contains("forbes.com");

    // Canonical link & Robots meta
    let canonical_url = doc
        .select(&Selector::parse("link[rel='canonical']").unwrap())
        .next()
        .and_then(|el| el.value().attr("href"))
        .map(|s| s.trim().to_string());
    let is_self_canonical = if let Some(ref c) = canonical_url {
        c.trim_end_matches('/') == page_url.trim_end_matches('/')
    } else {
        false
    };

    let robots_meta = doc
        .select(&Selector::parse("meta[name='robots'], meta[name='Robots']").unwrap())
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("")
        .to_lowercase();
    let is_noindex = robots_meta.contains("noindex");

    // Conversion signals: tel: links and forms
    let tel_links_count = doc
        .select(&Selector::parse("a[href^='tel:']").unwrap())
        .count();
    let form_count = doc.select(&Selector::parse("form").unwrap()).count();

    // Schema analysis & Ratings/Reviews
    let mut schema_types = Vec::new();
    let mut rating_value: Option<f64> = None;
    let mut review_count: Option<u64> = None;

    let json_ld_sel = Selector::parse("script[type='application/ld+json']").unwrap();
    for script in doc.select(&json_ld_sel) {
        let content = script.text().collect::<Vec<_>>().join("");
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            extract_schema_types(&val, &mut schema_types);
            extract_ratings_and_reviews(&val, &mut rating_value, &mut review_count);
        }
    }
    schema_types.sort();
    schema_types.dedup();

    let st_lower: Vec<String> = schema_types.iter().map(|s| s.to_lowercase()).collect();
    let has_local_business_schema = st_lower.iter().any(|s| {
        s.contains("localbusiness") || s.contains("attorney") || s.contains("legalservice")
    });
    let has_review_rating_schema = st_lower
        .iter()
        .any(|s| s.contains("aggregaterating") || s.contains("review"))
        || rating_value.is_some()
        || review_count.is_some();
    let has_faq_schema = st_lower
        .iter()
        .any(|s| s.contains("faqpage") || s.contains("question"));

    PageAuditResult {
        rank_label,
        rank_num,
        is_target,
        url: page_url.to_string(),
        domain,
        status: status_code,
        title_length: title.chars().count(),
        title,
        title_exact_match,
        title_starts_with_kw,
        meta_description_length: meta_desc.chars().count(),
        meta_description: meta_desc,
        meta_exact_match,
        h1,
        h1_exact_match,
        h2_count,
        h2_headings,
        h3_headings,
        questions_found,
        word_count,
        exact_keyword_count,
        keyword_density_pct,
        slug_has_exact_kw,
        slug_has_geo,
        is_dedicated_page,
        is_directory_aggregator,
        canonical_url,
        is_self_canonical,
        is_noindex,
        tel_links_count,
        form_count,
        schema_types,
        has_local_business_schema,
        has_review_rating_schema,
        has_faq_schema,
        review_count,
        rating_value,
        error: None,
    }
}

fn extract_schema_types(val: &serde_json::Value, out: &mut Vec<String>) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(t) = map.get("@type") {
                push_type_strings(t, out);
            }
            if let Some(graph) = map.get("@graph") {
                extract_schema_types(graph, out);
            }
            for (_k, v) in map {
                if v.is_object() || v.is_array() {
                    extract_schema_types(v, out);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_schema_types(item, out);
            }
        }
        _ => {}
    }
}

fn push_type_strings(val: &serde_json::Value, out: &mut Vec<String>) {
    match val {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let serde_json::Value::String(s) = item {
                    out.push(s.clone());
                }
            }
        }
        _ => {}
    }
}

fn parse_numeric_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn parse_numeric_u64(v: &serde_json::Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn extract_ratings_and_reviews(
    val: &serde_json::Value,
    rating: &mut Option<f64>,
    reviews: &mut Option<u64>,
) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(ar)) = map.get("aggregateRating") {
                if rating.is_none() {
                    *rating = ar.get("ratingValue").and_then(parse_numeric_f64);
                }
                if reviews.is_none() {
                    *reviews = ar
                        .get("reviewCount")
                        .or_else(|| ar.get("ratingCount"))
                        .and_then(parse_numeric_u64);
                }
            }

            if let Some(graph) = map.get("@graph") {
                extract_ratings_and_reviews(graph, rating, reviews);
            }
            for (_k, v) in map {
                if v.is_object() || v.is_array() {
                    extract_ratings_and_reviews(v, rating, reviews);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_ratings_and_reviews(item, rating, reviews);
            }
        }
        _ => {}
    }
}
