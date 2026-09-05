use scraper::Html;
use crate::core::feature_selectors::{extract_serp_features_by_selectors, SerpFeatureSelector};
use crate::core::types::{ResultType, SerpFeature};

const ECOSIA_FEATURE_SPECS: &[SerpFeatureSelector] = &[
    SerpFeatureSelector {
        feature_type: ResultType::AnswerBox,
        title: "",
        container: &[
            "[data-test-id='instant-answer']",
            "[data-test-id='answer-box']",
            ".instant-answer",
            "section[data-test-id='entity']",
            "div[data-test-id='entity']",
        ],
        title_selector: &["[data-test-id='entity-title']", "h2", "h3"],
        text_selector: &[
            "[data-test-id='entity-description']",
            ".instant-answer__text",
            ".instant-answer p",
            "p",
        ],
        item_selector: &[],
        link_selector: &["a[href^='http']"],
        position: 1,
        confidence: 0.8,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::RelatedSearches,
        title: "Related searches",
        container: &[
            "[data-test-id='web-related-queries']",
            ".related-queries__bottom",
            "[data-test-id='related-searches']",
            ".related-queries",
            "section:has([data-test-id='related-query'])",
        ],
        title_selector: &[],
        text_selector: &[],
        item_selector: &[
            "[data-test-id='related-query'] a",
            "a[data-test-id='related-query']",
            ".related-queries a",
            "li a",
            "a",
        ],
        link_selector: &["a[href^='http']", "a"],
        position: 0,
        confidence: 0.75,
        single_match: true,
    },
];

pub fn extract_ecosia_features(doc: &Html) -> Vec<SerpFeature> {
    extract_serp_features_by_selectors(doc, ECOSIA_FEATURE_SPECS)
}
