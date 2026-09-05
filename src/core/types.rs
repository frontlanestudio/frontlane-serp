use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const DEFAULT_QUERY_LIMIT: usize = 10;
pub const MAX_QUERY_LIMIT: usize = 100;
pub const DEFAULT_EXTRACT_TOP: usize = 1;
pub const MAX_EXTRACT_TOP: usize = 5;
pub const API_VERSION: &str = "2.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Json,
    Markdown,
    Text,
    Ndjson,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => OutputFormat::Markdown,
            "text" | "txt" => OutputFormat::Text,
            "ndjson" | "jsonl" => OutputFormat::Ndjson,
            _ => OutputFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub text: String,
    pub lang_code: String,
    pub region: String,
    pub date_interval: String,
    pub filetype: String,
    pub site: String,
    pub limit: usize,
    pub start: usize,
    pub filter: bool,
    pub features: bool,
    pub extract: bool,
    pub extract_top: usize,
    pub extract_mode: String,
    pub extract_min_runes: usize,
    pub proxy_url: Option<String>,
    pub proxy_country: Option<String>,
    pub proxy_class: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_session_id: Option<String>,
    pub proxy_override: Option<String>,
    pub insecure: bool,
    pub guard_private_networks: bool,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            text: String::new(),
            lang_code: String::new(),
            region: String::new(),
            date_interval: String::new(),
            filetype: String::new(),
            site: String::new(),
            limit: DEFAULT_QUERY_LIMIT,
            start: 0,
            filter: true,
            features: true,
            extract: false,
            extract_top: DEFAULT_EXTRACT_TOP,
            extract_mode: "auto".to_string(),
            extract_min_runes: 0,
            proxy_url: None,
            proxy_country: None,
            proxy_class: None,
            proxy_provider: None,
            proxy_session_id: None,
            proxy_override: None,
            insecure: false,
            guard_private_networks: true,
        }
    }
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.site.is_empty() && self.filetype.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResultType {
    #[default]
    Organic,
    Ad,
    FeaturedSnippet,
    KnowledgePanel,
    PeopleAlsoAsk,
    Video,
    Image,
    News,
    Shopping,
    Local,
    AnswerBox,
    AiSummary,
    RelatedQuestions,
    RelatedSearches,
    Sitelinks,
    Videos,
    ImagesInline,
    Calculator,
    Weather,
    Dictionary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub absolute: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tld: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sld: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_used: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub json_ld: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub meta_tags: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureLink {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerpFeature {
    pub id: String,
    pub engine: String,
    #[serde(rename = "type")]
    pub feature_type: ResultType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<FeatureItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<FeatureLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_result_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub extracted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub rank: i32,
    #[serde(default)]
    pub absolute_rank: i32,
    #[serde(default, rename = "type")]
    pub result_type: ResultType,
    pub url: String,
    pub title: String,
    pub description: String,
    pub ad: bool,
    #[serde(skip)]
    pub features: Vec<SerpFeature>,
    #[serde(skip)]
    pub image_data: Option<ImageData>,
    #[serde(skip)]
    pub image_source: Option<ImageSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultItem {
    pub id: String,
    pub rank: usize,
    #[serde(rename = "type")]
    pub result_type: ResultType,
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub snippet: String,
    pub domain: String,
    pub favicon: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub engine: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_info: Option<DomainInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<Classification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted: Option<ExtractedContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_consensus: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engines: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    pub page_url: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageResult {
    pub id: String,
    pub rank: usize,
    #[serde(rename = "type")]
    pub result_type: ResultType,
    pub title: String,
    pub image: ImageData,
    pub source: ImageSource,
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEcho {
    pub text: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lang: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub region: String,
    pub engines_requested: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineErrorDetail {
    pub engine: String,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub request_id: String,
    pub requested_at: String,
    pub took_ms: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engines_responded: Vec<String>,
    #[serde(default)]
    pub engines_failed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engine_errors: Vec<EngineErrorDetail>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub page: usize,
    pub has_more: bool,
    pub next_start: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterOccurrence {
    pub engine: String,
    pub rank: usize,
    pub result_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub id: String,
    pub canonical_url: String,
    pub domain: String,
    pub title: String,
    pub occurrences: Vec<ClusterOccurrence>,
    pub engines_count: usize,
    pub best_rank: usize,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub query: QueryEcho,
    pub meta: ResponseMeta,
    pub results: Vec<ResultItem>,
    pub serp_features: Vec<SerpFeature>,
    pub pagination: Pagination,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clusters: Option<Vec<Cluster>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEnvelope {
    pub query: QueryEcho,
    pub meta: ResponseMeta,
    pub results: Vec<ImageResult>,
    pub pagination: Pagination,
}

impl Envelope {
    pub fn new(
        q: &Query,
        request_id: String,
        started_at: DateTime<Utc>,
        engines: Vec<String>,
    ) -> Self {
        Self {
            query: QueryEcho {
                text: q.text.clone(),
                lang: q.lang_code.clone(),
                region: q.region.clone(),
                engines_requested: engines,
            },
            meta: ResponseMeta {
                request_id,
                requested_at: started_at.to_rfc3339(),
                took_ms: 0,
                engines_responded: Vec::new(),
                engines_failed: Vec::new(),
                engine_errors: Vec::new(),
                version: API_VERSION.to_string(),
            },
            results: Vec::new(),
            serp_features: Vec::new(),
            pagination: Pagination {
                page: 1,
                has_more: false,
                next_start: 0,
            },
            clusters: None,
        }
    }
}
