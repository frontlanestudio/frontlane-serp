use crate::core::types::SerpFeature;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RankStrategy {
    #[default]
    Smart,
    Basic,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DomainMatchMode {
    #[default]
    Subdomain,
    Exact,
    Wildcard,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    #[default]
    Desktop,
    Mobile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RankRequest {
    pub target: String,
    pub q: String,
    #[serde(default)]
    pub strategy: RankStrategy,
    #[serde(default)]
    pub last_rank: usize,
    #[serde(default = "default_pagination_limit")]
    pub pagination_limit: usize,
    #[serde(default)]
    pub smart_full_fallback: bool,
    #[serde(default)]
    pub r#match: DomainMatchMode,
    #[serde(default)]
    pub device: DeviceType,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub lang: String,
}

fn default_pagination_limit() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerpRankResultItem {
    pub rank: usize,
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default)]
    pub is_target: bool,
    #[serde(default)]
    pub is_directory: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankResponse {
    pub target: String,
    pub query: String,
    pub engine: String,
    pub device: String,
    pub ranked: bool,
    pub rank: Option<usize>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub serp_features: Vec<SerpFeature>,
    pub feature_citations: Vec<String>,
    pub pages_scraped: Vec<usize>,
    pub took_ms: i64,
    #[serde(default)]
    pub directory_count: usize,
    #[serde(default)]
    pub directory_share_pct: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ranking_directories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serp_results: Vec<SerpRankResultItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RankTarget {
    #[default]
    Domain,
    Subdomain,
    #[serde(alias = "exact")]
    ExactUrl,
    Directory,
}

impl RankTarget {
    pub fn to_match_mode(&self) -> DomainMatchMode {
        match self {
            RankTarget::Domain => DomainMatchMode::Exact,
            RankTarget::Subdomain => DomainMatchMode::Subdomain,
            RankTarget::ExactUrl => DomainMatchMode::Exact,
            RankTarget::Directory => DomainMatchMode::Directory,
        }
    }

    pub fn matches(&self, candidate_url: &str, target: &str) -> bool {
        crate::rank::matcher::matches_target(candidate_url, target, self.to_match_mode())
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            RankTarget::Domain => "Domain",
            RankTarget::Subdomain => "Subdomain",
            RankTarget::ExactUrl => "Exact URL",
            RankTarget::Directory => "Directory",
        }
    }
}

impl std::fmt::Display for RankTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RankTarget::Domain => write!(f, "domain"),
            RankTarget::Subdomain => write!(f, "subdomain"),
            RankTarget::ExactUrl => write!(f, "exact_url"),
            RankTarget::Directory => write!(f, "directory"),
        }
    }
}

impl From<RankTarget> for DomainMatchMode {
    fn from(target: RankTarget) -> Self {
        target.to_match_mode()
    }
}

impl From<DomainMatchMode> for RankTarget {
    fn from(mode: DomainMatchMode) -> Self {
        match mode {
            DomainMatchMode::Subdomain => RankTarget::Subdomain,
            DomainMatchMode::Exact => RankTarget::ExactUrl,
            DomainMatchMode::Wildcard => RankTarget::Domain,
            DomainMatchMode::Directory => RankTarget::Directory,
        }
    }
}

impl std::fmt::Display for DomainMatchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainMatchMode::Subdomain => write!(f, "subdomain"),
            DomainMatchMode::Exact => write!(f, "exact"),
            DomainMatchMode::Wildcard => write!(f, "wildcard"),
            DomainMatchMode::Directory => write!(f, "directory"),
        }
    }
}

impl std::fmt::Display for RankStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RankStrategy::Smart => write!(f, "smart"),
            RankStrategy::Basic => write!(f, "basic"),
            RankStrategy::Custom => write!(f, "custom"),
        }
    }
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceType::Desktop => write!(f, "desktop"),
            DeviceType::Mobile => write!(f, "mobile"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rank_targets_display() {
        assert_eq!(format!("{}", RankTarget::Domain), "domain");
        assert_eq!(format!("{}", RankTarget::Subdomain), "subdomain");
        assert_eq!(format!("{}", RankTarget::ExactUrl), "exact_url");
        assert_eq!(format!("{}", RankTarget::Directory), "directory");

        assert_eq!(RankTarget::Domain.display_name(), "Domain");
        assert_eq!(RankTarget::Subdomain.display_name(), "Subdomain");
        assert_eq!(RankTarget::ExactUrl.display_name(), "Exact URL");
        assert_eq!(RankTarget::Directory.display_name(), "Directory");

        assert_eq!(format!("{}", DomainMatchMode::Subdomain), "subdomain");
        assert_eq!(format!("{}", DomainMatchMode::Exact), "exact");
        assert_eq!(format!("{}", DomainMatchMode::Wildcard), "wildcard");
        assert_eq!(format!("{}", DomainMatchMode::Directory), "directory");

        assert_eq!(format!("{}", RankStrategy::Smart), "smart");
        assert_eq!(format!("{}", RankStrategy::Basic), "basic");
        assert_eq!(format!("{}", RankStrategy::Custom), "custom");

        assert_eq!(format!("{}", DeviceType::Desktop), "desktop");
        assert_eq!(format!("{}", DeviceType::Mobile), "mobile");
    }

    #[test]
    fn test_rank_targets_serialization() {
        // Serialization
        assert_eq!(
            serde_json::to_string(&RankTarget::Domain).unwrap(),
            "\"domain\""
        );
        assert_eq!(
            serde_json::to_string(&RankTarget::Subdomain).unwrap(),
            "\"subdomain\""
        );
        assert_eq!(
            serde_json::to_string(&RankTarget::ExactUrl).unwrap(),
            "\"exact_url\""
        );
        assert_eq!(
            serde_json::to_string(&RankTarget::Directory).unwrap(),
            "\"directory\""
        );

        // Deserialization
        assert_eq!(
            serde_json::from_str::<RankTarget>("\"domain\"").unwrap(),
            RankTarget::Domain
        );
        assert_eq!(
            serde_json::from_str::<RankTarget>("\"subdomain\"").unwrap(),
            RankTarget::Subdomain
        );
        assert_eq!(
            serde_json::from_str::<RankTarget>("\"exact_url\"").unwrap(),
            RankTarget::ExactUrl
        );
        // Supports alias "exact"
        assert_eq!(
            serde_json::from_str::<RankTarget>("\"exact\"").unwrap(),
            RankTarget::ExactUrl
        );
        assert_eq!(
            serde_json::from_str::<RankTarget>("\"directory\"").unwrap(),
            RankTarget::Directory
        );

        // DomainMatchMode serialization roundtrip
        for mode in [
            DomainMatchMode::Subdomain,
            DomainMatchMode::Exact,
            DomainMatchMode::Wildcard,
            DomainMatchMode::Directory,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: DomainMatchMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_rank_targets_matching() {
        // 1. RankTarget::Domain matches root domain only
        let target_domain = RankTarget::Domain;
        assert!(target_domain.matches("https://example.com/page", "example.com"));
        assert!(target_domain.matches("https://www.example.com", "example.com"));
        assert!(!target_domain.matches("https://blog.example.com", "example.com"));
        assert!(!target_domain.matches("https://another.com", "example.com"));

        // 2. RankTarget::Subdomain matches root and any subdomain
        let target_subdomain = RankTarget::Subdomain;
        assert!(target_subdomain.matches("https://example.com", "example.com"));
        assert!(target_subdomain.matches("https://blog.example.com", "example.com"));
        assert!(target_subdomain.matches("https://sub.blog.example.com/test", "example.com"));
        assert!(!target_subdomain.matches("https://notexample.com", "example.com"));

        // 3. RankTarget::ExactUrl matches exact path
        let target_exact = RankTarget::ExactUrl;
        assert!(target_exact.matches("https://example.com/services", "example.com/services"));
        assert!(target_exact.matches("https://example.com/services/", "example.com/services"));
        assert!(!target_exact.matches("https://example.com/about", "example.com/services"));
        assert!(!target_exact.matches("https://sub.example.com/services", "example.com/services"));

        // 4. RankTarget::Directory matches subpaths under directory
        let target_dir = RankTarget::Directory;
        assert!(target_dir.matches(
            "https://example.com/practice-areas/car-accidents",
            "example.com/practice-areas"
        ));
        assert!(target_dir.matches(
            "https://www.example.com/practice-areas/truck-accidents",
            "example.com/practice-areas"
        ));
        assert!(!target_dir.matches("https://example.com/about-us", "example.com/practice-areas"));
    }

    #[test]
    fn test_rank_conversions() {
        assert_eq!(
            DomainMatchMode::from(RankTarget::Domain),
            DomainMatchMode::Exact
        );
        assert_eq!(
            DomainMatchMode::from(RankTarget::Subdomain),
            DomainMatchMode::Subdomain
        );
        assert_eq!(
            DomainMatchMode::from(RankTarget::ExactUrl),
            DomainMatchMode::Exact
        );
        assert_eq!(
            DomainMatchMode::from(RankTarget::Directory),
            DomainMatchMode::Directory
        );

        assert_eq!(
            RankTarget::from(DomainMatchMode::Subdomain),
            RankTarget::Subdomain
        );
        assert_eq!(
            RankTarget::from(DomainMatchMode::Exact),
            RankTarget::ExactUrl
        );
        assert_eq!(
            RankTarget::from(DomainMatchMode::Directory),
            RankTarget::Directory
        );
    }

    #[test]
    fn test_rank_request_and_response_serialization() {
        let req = RankRequest {
            target: "example.com".to_string(),
            q: "best widgets".to_string(),
            strategy: RankStrategy::Custom,
            last_rank: 12,
            pagination_limit: 10,
            smart_full_fallback: true,
            r#match: DomainMatchMode::Directory,
            device: DeviceType::Mobile,
            region: "US".to_string(),
            lang: "en".to_string(),
        };

        let json = serde_json::to_string(&req).expect("RankRequest serialization failed");
        let parsed: RankRequest =
            serde_json::from_str(&json).expect("RankRequest deserialization failed");
        assert_eq!(parsed.target, "example.com");
        assert_eq!(parsed.q, "best widgets");
        assert_eq!(parsed.strategy, RankStrategy::Custom);
        assert_eq!(parsed.last_rank, 12);
        assert_eq!(parsed.pagination_limit, 10);
        assert!(parsed.smart_full_fallback);
        assert_eq!(parsed.r#match, DomainMatchMode::Directory);
        assert_eq!(parsed.device, DeviceType::Mobile);

        let item = SerpRankResultItem {
            rank: 1,
            url: "https://example.com/page".to_string(),
            title: "Test Page".to_string(),
            snippet: Some("Snippet text".to_string()),
            domain: Some("example.com".to_string()),
            is_target: true,
            is_directory: false,
            domain_category: Some("business".to_string()),
        };
        let item_json = serde_json::to_string(&item).unwrap();
        let parsed_item: SerpRankResultItem = serde_json::from_str(&item_json).unwrap();
        assert_eq!(parsed_item.rank, 1);
        assert!(parsed_item.is_target);
        assert_eq!(parsed_item.domain_category.as_deref(), Some("business"));
    }
}
