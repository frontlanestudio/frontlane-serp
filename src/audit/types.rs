use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageAuditResult {
    pub rank_label: String,
    pub rank_num: Option<usize>,
    pub is_target: bool,
    pub url: String,
    pub domain: String,
    pub status: u16,
    pub title: String,
    pub title_length: usize,
    pub title_exact_match: bool,
    pub title_starts_with_kw: bool,
    pub meta_description: String,
    pub meta_description_length: usize,
    pub meta_exact_match: bool,
    pub h1: Vec<String>,
    pub h1_exact_match: bool,
    pub h2_count: usize,
    pub word_count: usize,
    pub exact_keyword_count: usize,
    pub keyword_density_pct: f64,
    pub slug_has_exact_kw: bool,
    pub slug_has_geo: bool,
    pub is_dedicated_page: bool,
    pub schema_types: Vec<String>,
    pub has_local_business_schema: bool,
    pub has_review_rating_schema: bool,
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
    pub schema_adoption_pct: f64,
    pub common_schema_types: Vec<String>,
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
    pub timestamp: String,
}
