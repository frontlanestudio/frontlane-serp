use frontlane_serp::core::domain::{
    classify_content_type, enrich_domain_info, is_directory_domain,
};
use frontlane_serp::rank::{matches_target, DomainMatchMode, RankResponse, SerpRankResultItem};

#[test]
fn test_is_directory_domain_across_industries() {
    // Legal directories
    assert!(is_directory_domain("justia.com"));
    assert!(is_directory_domain("www.justia.com"));
    assert!(is_directory_domain("lawyers.justia.com"));
    assert!(is_directory_domain("avvo.com"));
    assert!(is_directory_domain("findlaw.com"));
    assert!(is_directory_domain("lawyers.com"));
    assert!(is_directory_domain("superlawyers.com"));
    assert!(is_directory_domain("nolo.com"));
    assert!(is_directory_domain("martindale.com"));
    assert!(is_directory_domain("martindale-avvo.com"));
    assert!(is_directory_domain("legal500.com"));
    assert!(is_directory_domain("bestlawyers.com"));

    // Local & Consumer directories
    assert!(is_directory_domain("yelp.com"));
    assert!(is_directory_domain("yellowpages.com"));
    assert!(is_directory_domain("bbb.org"));
    assert!(is_directory_domain("angi.com"));
    assert!(is_directory_domain("angis.com"));
    assert!(is_directory_domain("thumbtack.com"));
    assert!(is_directory_domain("tripadvisor.com"));
    assert!(is_directory_domain("manta.com"));
    assert!(is_directory_domain("expertise.com"));

    // Healthcare directories
    assert!(is_directory_domain("healthgrades.com"));
    assert!(is_directory_domain("zocdoc.com"));
    assert!(is_directory_domain("psychologytoday.com"));

    // B2B & Software directories
    assert!(is_directory_domain("g2.com"));
    assert!(is_directory_domain("capterra.com"));
    assert!(is_directory_domain("clutch.co"));

    // Real Estate directories
    assert!(is_directory_domain("zillow.com"));
    assert!(is_directory_domain("realtor.com"));

    // Non-directories should return false
    assert!(!is_directory_domain("google.com"));
    assert!(!is_directory_domain("calljacob.com"));
    assert!(!is_directory_domain("github.com"));
    assert!(!is_directory_domain("nytimes.com"));
    assert!(!is_directory_domain(""));
}

#[test]
fn test_domain_category_directory() {
    let justia_info = enrich_domain_info("justia.com").expect("DomainInfo should be returned");
    assert_eq!(justia_info.category, "directory");

    let yelp_info = enrich_domain_info("www.yelp.com").expect("DomainInfo should be returned");
    assert_eq!(yelp_info.category, "directory");

    let g2_info = enrich_domain_info("g2.com").expect("DomainInfo should be returned");
    assert_eq!(g2_info.category, "directory");

    let normal_info = enrich_domain_info("example.com").expect("DomainInfo should be returned");
    assert_ne!(normal_info.category, "directory");
}

#[test]
fn test_classify_content_type_directory_paths() {
    assert_eq!(
        classify_content_type("https://www.justia.com/lawyers/california/los-angeles"),
        "directory_listing"
    );
    assert_eq!(
        classify_content_type("https://www.yelp.com/biz/wilshire-law-firm-los-angeles"),
        "directory_listing"
    );
    assert_eq!(
        classify_content_type("https://www.findlaw.com/directory/attorneys"),
        "directory_listing"
    );
    assert_eq!(
        classify_content_type("https://www.clutch.co/companies/web-designers"),
        "directory_listing"
    );
    assert_eq!(
        classify_content_type("https://example.com/about-us"),
        "webpage"
    );
}

#[test]
fn test_matches_target_directory_mode() {
    // 1. Target directory path
    let target = "example.com/practice-areas/";

    // Direct matches inside target directory
    assert!(matches_target(
        "https://example.com/practice-areas/car-accidents/",
        target,
        DomainMatchMode::Directory
    ));
    assert!(matches_target(
        "https://www.example.com/practice-areas/truck-accidents",
        target,
        DomainMatchMode::Directory
    ));
    assert!(matches_target(
        "https://sub.example.com/practice-areas/sub-path/detail",
        target,
        DomainMatchMode::Directory
    ));
    assert!(matches_target(
        "https://example.com/practice-areas",
        target,
        DomainMatchMode::Directory
    ));

    // Non-matches
    assert!(!matches_target(
        "https://example.com/about-us",
        target,
        DomainMatchMode::Directory
    ));
    assert!(!matches_target(
        "https://example.com/practice-areas-other",
        target,
        DomainMatchMode::Directory
    ));
    assert!(!matches_target(
        "https://otherdomain.com/practice-areas/car-accidents",
        target,
        DomainMatchMode::Directory
    ));

    // 2. Tracking a directory aggregator silo
    let justia_target = "justia.com/lawyers/california";
    assert!(matches_target(
        "https://www.justia.com/lawyers/california/los-angeles",
        justia_target,
        DomainMatchMode::Directory
    ));
    assert!(matches_target(
        "https://lawyers.justia.com/lawyers/california",
        justia_target,
        DomainMatchMode::Directory
    ));
    assert!(!matches_target(
        "https://www.justia.com/lawyers/texas/houston",
        justia_target,
        DomainMatchMode::Directory
    ));

    // 3. Root directory path target matches any URL on domain
    let root_target = "example.com";
    assert!(matches_target(
        "https://example.com/anything/here",
        root_target,
        DomainMatchMode::Directory
    ));
}

#[test]
fn test_rank_response_directory_metrics_serialization() {
    let item = SerpRankResultItem {
        rank: 1,
        url: "https://www.justia.com/lawyers/california".to_string(),
        title: "California Lawyers - Justia".to_string(),
        snippet: Some("Top rated lawyers...".to_string()),
        domain: Some("justia.com".to_string()),
        is_target: false,
        is_directory: true,
        domain_category: Some("directory".to_string()),
    };

    let resp = RankResponse {
        target: "example.com".to_string(),
        query: "los angeles personal injury lawyer".to_string(),
        engine: "bing".to_string(),
        device: "desktop".to_string(),
        ranked: false,
        rank: None,
        url: None,
        title: None,
        serp_features: Vec::new(),
        feature_citations: Vec::new(),
        pages_scraped: vec![1],
        took_ms: 120,
        directory_count: 1,
        directory_share_pct: 100.0,
        ranking_directories: vec!["justia.com (#1)".to_string()],
        serp_results: vec![item],
    };

    let json_str = serde_json::to_string(&resp).expect("Serialization should succeed");
    assert!(json_str.contains("\"directory_count\":1"));
    assert!(json_str.contains("\"directory_share_pct\":100.0"));
    assert!(json_str.contains("\"is_directory\":true"));

    let deserialized: RankResponse =
        serde_json::from_str(&json_str).expect("Deserialization should succeed");
    assert_eq!(deserialized.directory_count, 1);
    assert_eq!(deserialized.directory_share_pct, 100.0);
    assert_eq!(deserialized.ranking_directories, vec!["justia.com (#1)"]);
    assert!(deserialized.serp_results[0].is_directory);
    assert_eq!(
        deserialized.serp_results[0].domain_category.as_deref(),
        Some("directory")
    );
}
