use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageAuditResult {
    pub rank_label: String,
    pub rank_num: Option<usize>,
    pub is_target: bool,
    pub url: String,
    pub domain: String,
    pub status: u16,

    // Title & Meta
    pub title: String,
    pub title_length: usize,
    pub title_exact_match: bool,
    pub title_starts_with_kw: bool,
    pub meta_description: String,
    pub meta_description_length: usize,
    pub meta_exact_match: bool,

    // Headings & Topical Outline
    pub h1: Vec<String>,
    pub h1_exact_match: bool,
    pub h2_count: usize,
    pub h2_headings: Vec<String>,
    pub h3_headings: Vec<String>,
    pub questions_found: Vec<String>,

    // Content Depth & Keywords
    pub word_count: usize,
    pub exact_keyword_count: usize,
    pub keyword_density_pct: f64,

    // URL & Architecture
    pub slug_has_exact_kw: bool,
    pub slug_has_geo: bool,
    pub is_dedicated_page: bool,
    pub is_directory_aggregator: bool,

    // Indexability & Canonical
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    pub is_self_canonical: bool,
    pub is_noindex: bool,

    // Conversion & Trust Signals
    pub tel_links_count: usize,
    pub form_count: usize,

    // Structured Schema & Reviews
    pub schema_types: Vec<String>,
    pub has_local_business_schema: bool,
    pub has_review_rating_schema: bool,
    pub has_faq_schema: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating_value: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SerpBenchmark {
    pub total_competitors_analyzed: usize,
    pub avg_word_count: usize,
    pub max_word_count: usize,
    pub min_word_count: usize,
    pub exact_title_match_pct: f64,
    pub exact_h1_match_pct: f64,
    pub dedicated_slug_pct: f64,
    pub directory_aggregator_pct: f64,
    pub schema_adoption_pct: f64,
    pub faq_schema_adoption_pct: f64,
    pub avg_tel_links: f64,
    pub avg_form_count: f64,
    pub common_schema_types: Vec<String>,
    pub top_competitor_topics: Vec<String>,
    pub competitor_questions: Vec<String>,
    pub has_local_pack_in_serp: bool,
    pub has_paa_in_serp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapInsight {
    pub category: String,
    pub severity: String, // "High", "Medium", "Low", "Opportunity"
    pub observation: String,
    pub actionable_recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordAuditReport {
    pub keyword: String,
    pub target_domain: String,
    pub target_url: Option<String>,
    pub engine: String,
    pub target_rank: Option<usize>,
    pub target_audit: Option<PageAuditResult>,
    pub competitor_audits: Vec<PageAuditResult>,
    pub benchmarks: SerpBenchmark,
    pub insights: Vec<GapInsight>,
    pub opportunity_score: u32,
    pub quick_wins: Vec<String>,
    pub missing_content_outline: Vec<String>,
    pub timestamp: String,
}
