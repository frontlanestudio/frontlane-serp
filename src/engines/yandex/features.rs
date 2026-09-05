use crate::core::feature_selectors::{extract_serp_features_by_selectors, SerpFeatureSelector};
use crate::core::types::{ResultType, SerpFeature};
use scraper::Html;

const YANDEX_FEATURE_SPECS: &[SerpFeatureSelector] = &[
    SerpFeatureSelector {
        feature_type: ResultType::AiSummary,
        title: "Нейро",
        container: &[
            "li[data-fast-name='neuro_answer']",
            ".FuturisSearch",
            ".FuturisSearchCard",
            "div[class*='FuturisSearch']",
            "div[data-bem*='neuro']",
        ],
        title_selector: &[".FuturisTitle", "h2", "h3"],
        text_selector: &[
            ".FuturisText",
            ".FuturisSearchCard-Text",
            ".FuturisSearch-Text",
            "div[class*='FuturisText']",
            "p",
        ],
        item_selector: &[],
        link_selector: &["a.FuturisSource", "a[href^='http']"],
        position: 1,
        confidence: 0.85,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::AnswerBox,
        title: "",
        container: &[
            ".FactAnswer",
            ".fact-answer",
            "li[data-fast-name='fact']",
            "li[data-fast-name='calculator']",
            "div[data-fast-name='fact']",
            "div[data-fast-name='calculator']",
            ".Organic-Fact",
            ".FactSnippet",
        ],
        title_selector: &[".FactAnswer-Title", ".Fact-Title", "h2"],
        text_selector: &[
            ".FactAnswer-Text",
            ".Fact-Answer",
            ".FactAnswer-Content",
            ".FactSnippet-Text",
            ".fact-answer__text",
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
            ".RelatedSearches",
            ".related",
            ".serp-footer__related",
            "div[data-fast-name='related']",
            ".Related-Items",
            ".Related-Content",
            "div[class*='RelatedSearches']",
        ],
        title_selector: &[],
        text_selector: &[],
        item_selector: &[
            ".Related-Item a",
            "a.Related-Item",
            ".RelatedSearches-Item a",
            "a[href*='/search?text=']",
            "a[href*='search/?text=']",
            "li a",
            "a",
        ],
        link_selector: &[
            "a[href*='/search?text=']",
            "a[href*='search/?text=']",
            "a[href^='http']",
        ],
        position: 0,
        confidence: 0.75,
        single_match: true,
    },
];

pub fn extract_yandex_features(doc: &Html) -> Vec<SerpFeature> {
    extract_serp_features_by_selectors(doc, YANDEX_FEATURE_SPECS)
}
