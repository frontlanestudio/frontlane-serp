use crate::core::feature_selectors::{extract_serp_features_by_selectors, SerpFeatureSelector};
use crate::core::types::{ResultType, SerpFeature};
use scraper::Html;

const DUCKDUCKGO_FEATURE_SPECS: &[SerpFeatureSelector] = &[
    SerpFeatureSelector {
        feature_type: ResultType::AiSummary,
        title: "AI Answer",
        container: &[
            "li[data-layout='wikinlp'] div.react-module",
            "[data-react-module-id='wikinlp']",
            "section[data-testid='duckassist']",
            "div[data-testid='duckassist']",
            ".react-duckassist",
            "#duckassist",
        ],
        title_selector: &["h2", "h3", "[data-testid='duckassist-title']"],
        text_selector: &[
            "[data-testid='duckassist-expanded-answer-content']",
            "[data-testid='duckassist-answer']",
            ".react-duckassist-snippet",
            "div.react-module",
            "p",
        ],
        item_selector: &[],
        link_selector: &["a[href^='http']"],
        position: 1,
        confidence: 0.8,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::AnswerBox,
        title: "",
        container: &[
            "#zero_click_wrapper",
            ".zci",
            ".zci--answer",
            ".result--answer",
            ".module--about",
            "[data-testid='instant-answer']",
            "section[data-testid='about']",
        ],
        title_selector: &[".c-base__title", "h2", "h3", ".module__title"],
        text_selector: &[
            ".js-about-item-abstr",
            ".zci__result",
            ".c-base__content",
            ".module__body",
            "p",
        ],
        item_selector: &[],
        link_selector: &["a[href^='http']"],
        position: 1,
        confidence: 0.8,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::RelatedQuestions,
        title: "People also ask",
        container: &[
            "[data-testid='related-questions']",
            ".related-questions",
            ".module--questions",
            "section:has([data-testid='related-question'])",
        ],
        title_selector: &["h2", "h3", ".module__title"],
        text_selector: &[],
        item_selector: &["[data-testid='related-question']", "li a", "button", "a"],
        link_selector: &["a[href^='http']"],
        position: 0,
        confidence: 0.7,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::RelatedSearches,
        title: "Related searches",
        container: &[
            "[data-testid='related-searches']",
            ".related-searches",
            ".result__related",
            "section:has(a[href*='?q='])",
            ".module--related-searches",
        ],
        title_selector: &[],
        text_selector: &[],
        item_selector: &[
            "a[href*='?q=']",
            "a[data-testid='related-search']",
            "li a",
            "a",
        ],
        link_selector: &["a[href*='?q=']", "a[href^='http']"],
        position: 0,
        confidence: 0.75,
        single_match: true,
    },
];

pub fn extract_duckduckgo_features(doc: &Html) -> Vec<SerpFeature> {
    extract_serp_features_by_selectors(doc, DUCKDUCKGO_FEATURE_SPECS)
}
