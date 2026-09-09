use frontlane_serp::audit::types::KeywordAuditReport;
use frontlane_serp::audit::{analyze_gaps, parse_page_audit, PageAuditResult};

#[test]
fn test_parse_page_audit_signals() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Best Rust Search Engine API</title>
            <meta name="description" content="Explore the best rust search engine API for crawlers.">
            <link rel="canonical" href="https://example.com/rust-search-engine">
            <script type="application/ld+json">
            {
                "@context": "https://schema.org",
                "@type": "LocalBusiness",
                "name": "Rust SERP Corp",
                "aggregateRating": {
                    "@type": "AggregateRating",
                    "ratingValue": "4.8",
                    "reviewCount": "120"
                }
            }
            </script>
        </head>
        <body>
            <h1>Best Rust Search Engine API</h1>
            <h2>Why Choose Async Rust?</h2>
            <h2>How Fast is SERP Scraping?</h2>
            <h3>Memory Efficiency</h3>
            <p>This is a complete guide to our rust search engine for fast rank tracking.</p>
            <a href="tel:+18005550199">Call Us</a>
            <form action="/contact" method="POST"><input type="text"/></form>
        </body>
        </html>
    "#;

    let audit = parse_page_audit(
        html,
        "https://example.com/rust-search-engine",
        "rust search engine",
        200,
        "1".to_string(),
        Some(1),
        true,
    );

    assert_eq!(audit.domain, "example.com");
    assert!(audit.title_exact_match);
    assert!(audit.meta_exact_match);
    assert!(audit.h1_exact_match);
    assert_eq!(audit.h2_count, 2);
    assert_eq!(audit.h2_headings.len(), 2);
    assert!(audit.h2_headings[0].contains("Why Choose Async Rust?"));
    assert!(audit
        .questions_found
        .iter()
        .any(|q| q.contains("Fast is SERP Scraping?")));
    assert_eq!(audit.tel_links_count, 1);
    assert_eq!(audit.form_count, 1);
    assert!(audit.has_local_business_schema);
    assert_eq!(audit.rating_value, Some(4.8));
    assert_eq!(audit.review_count, Some(120));
    assert!(audit.is_self_canonical);
}

#[test]
fn test_analyze_gaps_and_opportunity_scoring() {
    let target = PageAuditResult {
        rank_label: "Target".to_string(),
        rank_num: None,
        is_target: true,
        url: "https://mysite.com/page".to_string(),
        domain: "mysite.com".to_string(),
        status: 200,
        title: "Home Page".to_string(),
        title_length: 9,
        title_exact_match: false,
        title_starts_with_kw: false,
        meta_description: "Welcome to our site".to_string(),
        meta_description_length: 19,
        meta_exact_match: false,
        h1: vec!["Welcome".to_string()],
        h1_exact_match: false,
        h2_count: 1,
        h2_headings: vec!["About Us".to_string()],
        h3_headings: vec![],
        questions_found: vec![],
        word_count: 200,
        exact_keyword_count: 0,
        keyword_density_pct: 0.0,
        slug_has_exact_kw: false,
        slug_has_geo: false,
        is_dedicated_page: false,
        is_directory_aggregator: false,
        canonical_url: None,
        is_self_canonical: true,
        is_noindex: false,
        tel_links_count: 0,
        form_count: 0,
        schema_types: vec![],
        has_local_business_schema: false,
        has_review_rating_schema: false,
        has_faq_schema: false,
        review_count: None,
        rating_value: None,
        error: None,
    };

    let competitor = PageAuditResult {
        rank_label: "#1".to_string(),
        rank_num: Some(1),
        is_target: false,
        url: "https://competitor.com/rust-search-engine".to_string(),
        domain: "competitor.com".to_string(),
        status: 200,
        title: "Rust Search Engine - Top Rankings".to_string(),
        title_length: 32,
        title_exact_match: true,
        title_starts_with_kw: true,
        meta_description: "Detailed rust search engine guide".to_string(),
        meta_description_length: 33,
        meta_exact_match: true,
        h1: vec!["Rust Search Engine Architecture".to_string()],
        h1_exact_match: true,
        h2_count: 3,
        h2_headings: vec![
            "Pricing & Plans".to_string(),
            "Features & Specs".to_string(),
            "How to Integrate".to_string(),
        ],
        h3_headings: vec![],
        questions_found: vec!["How to Integrate".to_string()],
        word_count: 1500,
        exact_keyword_count: 8,
        keyword_density_pct: 0.53,
        slug_has_exact_kw: true,
        slug_has_geo: false,
        is_dedicated_page: true,
        is_directory_aggregator: false,
        canonical_url: None,
        is_self_canonical: true,
        is_noindex: false,
        tel_links_count: 2,
        form_count: 1,
        schema_types: vec!["LocalBusiness".to_string()],
        has_local_business_schema: true,
        has_review_rating_schema: true,
        has_faq_schema: true,
        review_count: Some(50),
        rating_value: Some(4.9),
        error: None,
    };

    let gaps = analyze_gaps(
        "rust search engine",
        Some(&target),
        &[competitor],
        true,
        true,
    );

    assert!(gaps.opportunity_score > 0);
    assert_eq!(gaps.benchmarks.avg_word_count, 1500);
    assert_eq!(gaps.benchmarks.max_word_count, 1500);
    assert_eq!(gaps.benchmarks.exact_title_match_pct, 100.0);
    assert!(!gaps.insights.is_empty());
    assert!(!gaps.quick_wins.is_empty());
    assert!(!gaps.missing_content_outline.is_empty());
}

#[test]
fn test_audit_export_formats() {
    let target = PageAuditResult {
        rank_label: "Target".to_string(),
        rank_num: Some(3),
        is_target: true,
        url: "https://example.com/rust".to_string(),
        domain: "example.com".to_string(),
        status: 200,
        title: "Rust Search".to_string(),
        title_length: 11,
        title_exact_match: true,
        title_starts_with_kw: true,
        meta_description: "Rust search guide".to_string(),
        meta_description_length: 17,
        meta_exact_match: true,
        h1: vec!["Rust Search Guide".to_string()],
        h1_exact_match: true,
        h2_count: 2,
        h2_headings: vec!["Getting Started".to_string()],
        h3_headings: vec![],
        questions_found: vec![],
        word_count: 800,
        exact_keyword_count: 3,
        keyword_density_pct: 0.37,
        slug_has_exact_kw: true,
        slug_has_geo: false,
        is_dedicated_page: true,
        is_directory_aggregator: false,
        canonical_url: None,
        is_self_canonical: true,
        is_noindex: false,
        tel_links_count: 1,
        form_count: 1,
        schema_types: vec!["FAQPage".to_string()],
        has_local_business_schema: false,
        has_review_rating_schema: false,
        has_faq_schema: true,
        review_count: None,
        rating_value: None,
        error: None,
    };

    let report = KeywordAuditReport {
        keyword: "rust search".to_string(),
        target_domain: "example.com".to_string(),
        target_url: Some("https://example.com/rust".to_string()),
        engine: "google".to_string(),
        target_rank: Some(3),
        target_audit: Some(target),
        competitor_audits: vec![],
        benchmarks: Default::default(),
        insights: vec![],
        opportunity_score: 75,
        quick_wins: vec!["Add LocalBusiness schema".to_string()],
        missing_content_outline: vec!["Architecture Overview".to_string()],
        timestamp: "2026-09-06T00:00:00Z".to_string(),
    };

    let json_str = report.to_json().expect("to_json");
    assert!(json_str.contains(r#""keyword": "rust search""#));
    assert!(json_str.contains(r#""opportunity_score": 75"#));

    let csv_str = report.to_csv();
    assert!(csv_str.contains("keyword,rank,is_target,domain,url"));
    assert!(csv_str.contains("\"rust search\",\"Target\",true,\"example.com\""));

    let md_str = report.to_markdown();
    assert!(md_str.contains("# SERP Competitor Audit: 'rust search'"));
    assert!(md_str.contains("**Opportunity Score**: **75/100**"));
    assert!(md_str.contains("Add LocalBusiness schema"));

    let console_txt = report.to_console_text();
    assert!(console_txt.contains("SERP AUDIT & COMPETITOR GAP ANALYSIS: 'rust search'"));
}
