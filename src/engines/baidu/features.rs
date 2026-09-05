use crate::core::feature_selectors::{extract_serp_features_by_selectors, SerpFeatureSelector};
use crate::core::types::{ResultType, SerpFeature};
use scraper::Html;

const BAIDU_FEATURE_SPECS: &[SerpFeatureSelector] = &[
    SerpFeatureSelector {
        feature_type: ResultType::AiSummary,
        title: "AI智能回答",
        container: &[
            "div[tpl='app/chat-input']",
            "div[tpl='ai_chat']",
            "div[tpl*='ai']",
            ".op-ai-answer",
            ".cosc-result",
            ".ai-answer",
            "div[class*='ai-chat']",
        ],
        title_selector: &[".c-title", "h3", "h2"],
        text_selector: &[
            ".cosc-content",
            ".cosc-text",
            ".op-ai-answer-content",
            ".ai-answer-text",
            ".c-abstract",
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
            ".op_exactqa_s_answer",
            ".op_dict_content",
            ".op_weather4_twoicon",
            "div[tpl='calculator']",
            "div[tpl='app/calc']",
            ".op_exactqa_detail",
            ".op_sp_exactqa",
        ],
        title_selector: &[".c-title", "h3", ".op_exactqa_title"],
        text_selector: &[
            ".op_exactqa_s_answer",
            ".op_exactqa_detail",
            ".op_dict_content",
            ".c-abstract",
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
        title: "相关搜索",
        container: &[
            "div[tpl='app/rs']",
            "#rs_new",
            "#rs",
            ".opr-recommends-merge-content",
            ".c-recommend",
            ".rs_table",
        ],
        title_selector: &[],
        text_selector: &[],
        item_selector: &[
            "table a",
            ".opr-recommends-merge-p a",
            "a[href*='s?wd=']",
            "a[href*='baidu.com/s?']",
            "a",
        ],
        link_selector: &[
            "a[href*='s?wd=']",
            "a[href*='baidu.com/s?']",
            "a[href^='http']",
        ],
        position: 0,
        confidence: 0.75,
        single_match: true,
    },
];

pub fn extract_baidu_features(doc: &Html) -> Vec<SerpFeature> {
    extract_serp_features_by_selectors(doc, BAIDU_FEATURE_SPECS)
}
