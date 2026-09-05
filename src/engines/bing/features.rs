use crate::core::feature_selectors::{extract_serp_features_by_selectors, SerpFeatureSelector};
use crate::core::types::{ResultType, SerpFeature};
use scraper::Html;

const BING_FEATURE_SPECS: &[SerpFeatureSelector] = &[
    SerpFeatureSelector {
        feature_type: ResultType::AnswerBox,
        title: "",
        container: &[
            "li.b_ans:has(.b_focusTextLarge)",
            "li.b_ans:has(.b_focusLabel)",
            "li.b_ans:has(.b_xlText)",
            "li.b_ans:has(.b_factrow)",
        ],
        title_selector: &[".b_focusLabel", "h2"],
        text_selector: &[
            ".b_focusTextLarge",
            ".b_xlText",
            ".b_vPanel .b_factrow",
            ".b_caption p",
        ],
        item_selector: &[],
        link_selector: &["a[href^='http']"],
        position: 1,
        confidence: 0.8,
        single_match: false,
    },
    SerpFeatureSelector {
        feature_type: ResultType::RelatedQuestions,
        title: "People also ask",
        container: &[".b_rrsr", ".rqnaacfacc", "li.b_ans:has(.df_alaskcr)"],
        title_selector: &[],
        text_selector: &[],
        item_selector: &[".df_qntext", ".rqnaacfacc a", "li a"],
        link_selector: &["a[href^='http']"],
        position: 0,
        confidence: 0.7,
        single_match: false,
    },
    SerpFeatureSelector {
        feature_type: ResultType::RelatedSearches,
        title: "Related searches",
        container: &[
            "#brsv3",
            "#rs_root",
            "#inline_rs",
            "#brs",
            "#b_rs",
            "ol#b_rs",
            "li.b_rs",
        ],
        title_selector: &[],
        text_selector: &[],
        item_selector: &["li.rslist a", "li a", "a"],
        link_selector: &["a[href^='http']", "a"],
        position: 0,
        confidence: 0.75,
        single_match: true,
    },
    SerpFeatureSelector {
        feature_type: ResultType::AiSummary,
        title: "AI answer",
        container: &[
            ".developer_answercard_wrapper",
            "#ca_main",
            ".ca_container",
            "#b_sydConvCont",
            ".b_sydConvCont",
            "[data-testid='bing-chat-answer']",
        ],
        title_selector: &["h2.b_topTitle", ".b_sydAns"],
        text_selector: &[
            ".devmag_card_content",
            ".rd_def_list",
            ".b_sydAns",
            "[data-testid='answer']",
            ".ca_div",
            "p",
        ],
        item_selector: &[],
        link_selector: &[
            ".rd_cnt_srcs a[href^='http']",
            ".rd_gencon_attr a[href^='http']",
            "h2.b_topTitle a[href^='http']",
            "a[href^='http']",
        ],
        position: 1,
        confidence: 0.6,
        single_match: false,
    },
];

pub fn extract_bing_features(doc: &Html) -> Vec<SerpFeature> {
    let mut features = extract_serp_features_by_selectors(doc, BING_FEATURE_SPECS);
    // Filter out related searches leaked into answer_box
    features.retain(|f| {
        if f.feature_type == ResultType::AnswerBox {
            let title = f.title.as_deref().unwrap_or("").to_lowercase();
            !title.contains("searches you might like")
        } else {
            true
        }
    });
    features
}
