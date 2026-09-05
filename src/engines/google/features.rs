use crate::core::feature_selectors::{extract_serp_features_by_selectors, SerpFeatureSelector};
use crate::core::types::{ResultType, SerpFeature};
use scraper::Html;

const GOOGLE_FEATURE_SPECS: &[SerpFeatureSelector] = &[
    SerpFeatureSelector {
        feature_type: ResultType::AiSummary,
        title: "AI Overview",
        container: &[
            "div[data-mcpr]:has(div[data-subtree='aimc'])",
            "div[data-container-id='main-col'][data-sfc-root='c']",
            "div[data-subtree='aifb']",
            "div[data-mcpr]",
            "div[aria-label*='AI Overview']",
            "div[jsname][data-rl]",
            "div[data-rsoextract]",
        ],
        title_selector: &["[role='heading']", "h2", "h3"],
        text_selector: &[
            "div[data-subtree='aimc']",
            "div[data-streaming-container]",
            "div[data-sncf='1']",
            "[data-attrid*='description']",
            "div[data-subtree='aifb']",
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
];

pub fn extract_google_features(doc: &Html) -> Vec<SerpFeature> {
    let raw_features = extract_serp_features_by_selectors(doc, GOOGLE_FEATURE_SPECS);
    filter_google_placeholders(raw_features)
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
