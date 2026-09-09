use async_trait::async_trait;
use frontlane_serp::core::engine::SearchEngine;
use frontlane_serp::core::error::Result;
use frontlane_serp::core::types::{Query, ResultType, SearchResult};
use frontlane_serp::mega::MegaSearcher;
use std::sync::Arc;

struct MockEngine {
    name: &'static str,
    results: Vec<SearchResult>,
}

#[async_trait]
impl SearchEngine for MockEngine {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn search(&self, _query: &Query) -> Result<Vec<SearchResult>> {
        Ok(self.results.clone())
    }

    async fn search_image(&self, _query: &Query) -> Result<Vec<SearchResult>> {
        Ok(vec![])
    }
}

fn mock_result(rank: i32, url: &str, title: &str, desc: &str) -> SearchResult {
    SearchResult {
        rank,
        absolute_rank: rank,
        result_type: ResultType::Organic,
        url: url.to_string(),
        title: title.to_string(),
        description: desc.to_string(),
        ad: false,
        features: vec![],
        image_data: None,
        image_source: None,
    }
}

#[tokio::test]
async fn test_megasearch_dedup_and_clusters() {
    let engine_a = Arc::new(MockEngine {
        name: "mock_a",
        results: vec![
            mock_result(
                1,
                "https://example.com/rust",
                "Rust Lang",
                "Rust programming language",
            ),
            mock_result(
                2,
                "https://example.com/go",
                "Go Lang",
                "Go programming language",
            ),
        ],
    });

    let engine_b = Arc::new(MockEngine {
        name: "mock_b",
        results: vec![
            mock_result(
                1,
                "https://example.com/rust?utm_source=test",
                "Rust Lang Home",
                "Fast, reliable language",
            ),
            mock_result(
                2,
                "https://example.com/python",
                "Python Lang",
                "Python language",
            ),
        ],
    });

    let mega = MegaSearcher::new(vec![engine_a, engine_b], None);
    let q = Query {
        text: "programming languages".to_string(),
        lang_code: "".to_string(),
        region: "".to_string(),
        date_interval: "".to_string(),
        filetype: "".to_string(),
        site: "".to_string(),
        limit: 10,
        start: 0,
        filter: true,
        features: true,
        extract: false,
        extract_top: 0,
        extract_mode: "auto".to_string(),
        extract_min_runes: 0,
        proxy_url: None,
        proxy_country: None,
        proxy_class: None,
        proxy_provider: None,
        proxy_session_id: None,
        proxy_override: None,
        insecure: true,
        guard_private_networks: false,
    };

    let envelope = mega
        .search(
            &q,
            &["mock_a".to_string(), "mock_b".to_string()],
            "balanced",
        )
        .await
        .expect("megasearch should succeed");

    // https://example.com/rust and https://example.com/rust?utm_source=test should deduplicate to 1 result
    assert_eq!(envelope.results.len(), 3);

    // Top result should be rust with consensus = 2 and highest RRF score
    let top = &envelope.results[0];
    assert!(top.url.contains("example.com/rust"));
    assert_eq!(top.rank, 1);
    assert_eq!(top.engine_consensus, Some(2));
    assert!(top.score.is_some());
    assert!(top.score.unwrap() > 0.03); // 2 / (60 + 1) = 0.0328
    let engines = top.engines.as_ref().expect("engines list present");
    assert!(engines.contains(&"mock_a".to_string()));
    assert!(engines.contains(&"mock_b".to_string()));

    // Clusters should group the rust result from both engines
    let clusters = envelope.clusters.expect("clusters must be present");
    let rust_cluster = clusters
        .iter()
        .find(|c| c.canonical_url.contains("example.com/rust"))
        .expect("rust cluster must exist");

    assert_eq!(rust_cluster.engines_count, 2);
    assert_eq!(rust_cluster.occurrences.len(), 2);
    // score = (1/1 + 1/1) / 2 = 1.0
    assert!((rust_cluster.score - 1.0).abs() < f64::EPSILON);
}
