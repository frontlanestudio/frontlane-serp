use crate::core::feature_selectors::{extract_serp_features_by_selectors, SerpFeatureSelector};
use crate::core::types::{FeatureItem, FeatureLink, GeoCoordinates, Position, ResultType, SerpFeature};
use chrono::Utc;
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;
use url::Url;

static RATING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([0-5]\.[0-9])").unwrap());
static REVIEWS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(?([0-9,]+)\s*(?:reviews|ratings)?\)?").unwrap());
static COORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:@|[?&](?:ll|center)=@?)(-?\d+\.\d+),(-?\d+\.\d+)").unwrap());
static CID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"ludocid=([0-9]+)").unwrap());

const GOOGLE_FEATURE_SPECS: &[SerpFeatureSelector] = &[
    SerpFeatureSelector {
        feature_type: ResultType::AiSummary,
        title: "AI Overview",
        container: &[
            "div[data-attrid='wa_overview']",
            "div[data-mcpr]:has(div[data-subtree='aimc'])",
            "div[data-container-id='main-col'][data-sfc-root='c']",
            "div[data-subtree='aifb']",
            "div[data-mcpr]",
            "div[aria-label*='AI Overview']",
            "div[jsname][data-rl]",
            "div[data-rsoextract]",
            "div.cUnQKe",
        ],
        title_selector: &["[role='heading']", "h2", "h3"],
        text_selector: &[
            "div[data-subtree='aimc']",
            "div[data-streaming-container]",
            "div[data-sncf='1']",
            "[data-attrid*='description']",
            "div[data-subtree='aifb']",
            "div.NFmi1e",
        ],
        item_selector: &[],
        link_selector: &[
            "div[data-subtree='aimc'] a[href^='http']",
            "a[href^='http']",
        ],
        position: 1,
        confidence: 0.75,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::PeopleAlsoAsk,
        title: "People also ask",
        container: &["div[data-initq]", "div[jsname='yEVEwb']"],
        title_selector: &[],
        text_selector: &[],
        item_selector: &["div.related-question-pair[data-q]", "div[data-q]"],
        link_selector: &["a[href^='http']"],
        position: 1,
        confidence: 0.8,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::RelatedSearches,
        title: "Related searches",
        container: &[
            "div[jsname='yEVEwb'][role='navigation']",
            "div[data-abe='1']",
        ],
        title_selector: &[],
        text_selector: &[],
        item_selector: &["a[href*='/search?']"],
        link_selector: &["a[href*='/search?']"],
        position: 0,
        confidence: 0.6,
        single_match: false,
    },
    SerpFeatureSelector {
        feature_type: ResultType::LocalServicesAds,
        title: "Google Guaranteed / Local Services Ads",
        container: &[
            "div[data-attrid*='local_services_ads']",
            "div[data-attrid*='kc:/location/location:local_services_ads']",
            "div.xpdclose:has(div.uE30Ze)",
            "div.GLkC4d",
        ],
        title_selector: &["div[role='heading']", "h2", "h3"],
        text_selector: &["span.r2WbTd", "div.z5rAub"],
        item_selector: &[
            "div.uE30Ze",
            "div.vw5Aee",
            "div.kno-fb-ctx",
            "div[data-record-click-time]",
        ],
        link_selector: &["a[href^='http']", "a[data-ved]"],
        position: 0,
        confidence: 0.85,
        single_match: true,
    },
];

pub fn extract_google_features(doc: &Html) -> Vec<SerpFeature> {
    let mut features = Vec::new();

    // 1. Deep AI Overview extraction (SGE & GEO Citations)
    if let Some(ai_overview) = extract_deep_ai_overview(doc) {
        features.push(ai_overview);
    }

    // 2. Deep Local 3-Pack extraction (ratings, reviews, CID, Place ID, coordinates)
    if let Some(local_pack) = extract_deep_local_pack(doc) {
        features.push(local_pack);
    }

    // 3. Extract standard features (PAA, Related searches, LSA)
    let raw_features = extract_serp_features_by_selectors(doc, GOOGLE_FEATURE_SPECS);
    for f in filter_google_placeholders(raw_features) {
        // Don't duplicate AI Overview if we already extracted a deep one
        if f.feature_type == ResultType::AiSummary && features.iter().any(|existing| existing.feature_type == ResultType::AiSummary) {
            continue;
        }
        features.push(f);
    }

    features
}

/// Extracts rich Google AI Overview (SGE) including synthesized sections and source citations.
pub fn extract_deep_ai_overview(doc: &Html) -> Option<SerpFeature> {
    let container_selectors = [
        "div[data-attrid='wa_overview']",
        "div[data-mcpr]:has(div[data-subtree='aimc'])",
        "div[data-container-id='main-col'][data-sfc-root='c']",
        "div[data-subtree='aimc']",
        "div.cUnQKe",
        "div[aria-label*='AI Overview']",
    ];

    let mut container_el = None;
    for sel_str in &container_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = doc.select(&sel).next() {
                container_el = Some(el);
                break;
            }
        }
    }

    let container = container_el?;

    // Extract text sections & synthesized paragraphs
    let mut text_parts = Vec::new();
    let text_sel = Selector::parse("div[data-subtree='aimc'], div[data-sncf='1'], div.NFmi1e, p, li").ok()?;
    for el in container.select(&text_sel) {
        let txt = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
        if !txt.is_empty() && !text_parts.iter().any(|existing: &String| existing.contains(&txt) || txt.contains(existing)) {
            if !looks_like_css(&txt) && !is_placeholder_text(&txt) {
                text_parts.push(txt);
            }
        }
    }

    if text_parts.is_empty() {
        return None;
    }

    let full_text = text_parts.join("\n\n");

    // Extract citations (sources)
    let mut citations = Vec::new();
    let mut links = Vec::new();
    let anchor_sel = Selector::parse("a[href^='http']").ok()?;

    for a in container.select(&anchor_sel) {
        if let Some(href) = a.value().attr("href") {
            let href = href.trim();
            if href.is_empty() || href.contains("google.com/search") || href.contains("google.com/url?") {
                continue;
            }

            let link_title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let domain = Url::parse(href).ok().and_then(|u| u.domain().map(|d| d.to_string()));

            if !links.iter().any(|l: &FeatureLink| l.url.as_deref() == Some(href)) {
                links.push(FeatureLink {
                    title: if link_title.is_empty() { None } else { Some(link_title.clone()) },
                    url: Some(href.to_string()),
                });

                citations.push(FeatureItem {
                    title: if link_title.is_empty() { None } else { Some(link_title) },
                    link: Some(href.to_string()),
                    domain,
                    snippet: None,
                    text: None,
                    rating: None,
                    reviews_count: None,
                    place_id: None,
                    cid: None,
                    address: None,
                    hours: None,
                    coordinates: None,
                });
            }
        }
    }

    Some(SerpFeature {
        id: format!("google-ai-overview-{}", Utc::now().timestamp_micros()),
        engine: "google".to_string(),
        feature_type: ResultType::AiSummary,
        title: Some("AI Overview".to_string()),
        text: Some(full_text),
        items: citations,
        links,
        source_result_ids: Vec::new(),
        position: Some(Position { absolute: 1 }),
        confidence: Some(0.95),
        extracted_at: Utc::now().to_rfc3339(),
    })
}

/// Extracts Google Local 3-Pack entities including CID, Place ID, ratings, reviews, coordinates, and hours.
pub fn extract_deep_local_pack(doc: &Html) -> Option<SerpFeature> {
    let card_selectors = [
        "div.VkpGBb",
        "div.cXedhc",
        "div[data-cid]",
        "div[data-attrid*='local_pack'] div.uE30Ze",
    ];

    let mut card_elements = Vec::new();
    for sel_str in &card_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in doc.select(&sel) {
                card_elements.push(el);
            }
        }
        if !card_elements.is_empty() {
            break;
        }
    }

    if card_elements.is_empty() {
        return None;
    }

    let title_sel = Selector::parse("div.dbg0pd, [role='heading'], span.OSrXXb, div.fontHeadlineSmall, span.rllt__wrapped").ok()?;
    let rating_sel = Selector::parse("span.yi40Hd, span[aria-label*='stars'], span.YrbPuc").ok()?;
    let reviews_sel = Selector::parse("span.RDApEe, span[aria-label*='reviews']").ok()?;
    let details_sel = Selector::parse("div.rllt__details > div, div.rllt__details span, span.W4Efsd, div.rllt__details").ok()?;
    let anchor_sel = Selector::parse("a[href]").ok()?;

    let mut local_items = Vec::new();

    for card in card_elements {
        // Business name / title
        let title = card
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .filter(|t| !t.is_empty());

        let Some(business_title) = title else {
            continue;
        };

        // Rating
        let mut rating = None;
        if let Some(r_el) = card.select(&rating_sel).next() {
            let r_text = r_el.text().collect::<Vec<_>>().join(" ");
            if let Some(caps) = RATING_RE.captures(&r_text) {
                rating = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
            } else if let Some(aria) = r_el.value().attr("aria-label") {
                if let Some(caps) = RATING_RE.captures(aria) {
                    rating = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
                }
            }
        }

        // Reviews count
        let mut reviews_count = None;
        if let Some(rv_el) = card.select(&reviews_sel).next() {
            let rv_text = rv_el.text().collect::<Vec<_>>().join(" ");
            if let Some(caps) = REVIEWS_RE.captures(&rv_text) {
                reviews_count = caps.get(1).and_then(|m| m.as_str().replace(',', "").parse::<usize>().ok());
            }
        }

        // Details / Address / Hours
        let mut address = None;
        let mut hours = None;
        for d_el in card.select(&details_sel) {
            let text = d_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if text.is_empty() {
                continue;
            }
            if text.contains("Open") || text.contains("Closed") || text.contains("Closes") {
                if hours.is_none() {
                    hours = Some(text);
                }
            } else if address.is_none() && (text.contains("·") || text.chars().any(|c| c.is_ascii_digit())) {
                address = Some(text);
            }
        }

        // CID
        let mut cid = card.value().attr("data-cid").map(|s| s.to_string());
        let place_id = card.value().attr("data-place-id").or_else(|| card.value().attr("data-pid")).map(|s| s.to_string());

        // Link & Coordinates
        let mut link = None;
        let mut coordinates = None;

        for a_el in card.select(&anchor_sel) {
            if let Some(href) = a_el.value().attr("href") {
                if link.is_none() {
                    link = Some(href.to_string());
                }

                if cid.is_none() {
                    if let Some(caps) = CID_RE.captures(href) {
                        cid = caps.get(1).map(|m| m.as_str().to_string());
                    }
                }

                if coordinates.is_none() {
                    if let Some(caps) = COORD_RE.captures(href) {
                        if let (Some(lat_m), Some(lng_m)) = (caps.get(1), caps.get(2)) {
                            if let (Ok(lat), Ok(lng)) = (lat_m.as_str().parse::<f64>(), lng_m.as_str().parse::<f64>()) {
                                coordinates = Some(GeoCoordinates { lat, lng });
                            }
                        }
                    }
                }
            }
        }

        // Also check data-lat / data-lng attributes
        if coordinates.is_none() {
            if let (Some(lat_str), Some(lng_str)) = (card.value().attr("data-lat"), card.value().attr("data-lng")) {
                if let (Ok(lat), Ok(lng)) = (lat_str.parse::<f64>(), lng_str.parse::<f64>()) {
                    coordinates = Some(GeoCoordinates { lat, lng });
                }
            }
        }

        let domain = link.as_ref().and_then(|l| Url::parse(l).ok()).and_then(|u| u.domain().map(|d| d.to_string()));

        local_items.push(FeatureItem {
            title: Some(business_title),
            text: None,
            link,
            domain,
            snippet: None,
            rating,
            reviews_count,
            place_id,
            cid,
            address,
            hours,
            coordinates,
        });
    }

    if local_items.is_empty() {
        return None;
    }

    Some(SerpFeature {
        id: format!("google-local-pack-{}", Utc::now().timestamp_micros()),
        engine: "google".to_string(),
        feature_type: ResultType::Local,
        title: Some("Local 3-Pack".to_string()),
        text: None,
        items: local_items,
        links: Vec::new(),
        source_result_ids: Vec::new(),
        position: Some(Position { absolute: 1 }),
        confidence: Some(0.9),
        extracted_at: Utc::now().to_rfc3339(),
    })
}

fn filter_google_placeholders(features: Vec<SerpFeature>) -> Vec<SerpFeature> {
    features
        .into_iter()
        .filter(|f| {
            if f.feature_type == ResultType::AiSummary {
                !is_google_placeholder(f)
            } else {
                true
            }
        })
        .collect()
}

fn is_google_placeholder(f: &SerpFeature) -> bool {
    let Some(ref text) = f.text else { return true };
    is_placeholder_text(text)
}

fn is_placeholder_text(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    if lower.is_empty() || lower == "show more" || lower == "show less" {
        return true;
    }
    if looks_like_css(&lower) {
        return true;
    }
    const PLACEHOLDERS: &[&str] = &[
        "ai overview is not available",
        "an ai overview is not available for this search",
        "обзор от ии недоступен",
    ];
    for p in PLACEHOLDERS {
        if lower.contains(p) {
            return true;
        }
    }
    false
}

fn looks_like_css(text: &str) -> bool {
    if text.contains("@keyframes") || text.contains("@media") {
        return true;
    }
    if text.contains("} .") || text.contains("} #") {
        return true;
    }
    text.contains("{ ") && text.matches(": ").count() > 20 && text.matches(';').count() > 20
}
