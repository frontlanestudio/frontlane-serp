use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use frontlane_serp::config::AppConfig;
use frontlane_serp::core::http_client::HttpClient;
use frontlane_serp::server::create_router;
use frontlane_serp::server::state::AppState;

fn create_test_app() -> axum::Router {
    let config = AppConfig::default();
    let http_client = HttpClient::new(None, true, 10).expect("http client");
    let state = AppState::new(config, vec![], http_client);
    create_router(state)
}

#[tokio::test]
async fn test_server_routes_health_and_root() {
    let app = create_test_app();

    // Test GET /
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("frontlane-serp"));

    // Test GET /health
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("healthy"));
}

#[tokio::test]
async fn test_server_parse_google_endpoint() {
    let app = create_test_app();

    let sample_html = std::fs::read_to_string("tests/fixtures/google/search_results.html")
        .expect("read google fixture");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/google/parse")
                .header("content-type", "text/html")
                .body(Body::from(sample_html))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("results"));
    assert!(body_str.contains("MDN Web Docs") || body_str.contains("test.test"));
}

#[tokio::test]
async fn test_server_rank_validation() {
    let app = create_test_app();

    // Missing target/q should return 400
    let response = app
        .oneshot(
            Request::builder()
                .uri("/google/rank")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_server_suggest_validation() {
    let app = create_test_app();

    // Missing q parameter should return 400
    let response = app
        .oneshot(
            Request::builder()
                .uri("/google/suggest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_server_jobs_routes() {
    let app = create_test_app();

    // Non-existent job returns 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/jobs/job_does_not_exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Empty batch returns 400
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/rank/batch")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"targets": []}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_server_stats_routes() {
    let app = create_test_app();

    // 1. Test /stats
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(val.get("cache").is_some());
    assert!(val.get("proxy").is_some());
    assert!(val.get("circuit_breakers").is_some());
    assert!(val.get("captcha").is_some());

    // 2. Test /stats/cache
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/stats/cache")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(val.get("entry_count").is_some());

    // 3. Test /stats/proxy
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/stats/proxy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(val.get("allow_request_proxy_url").is_some());
    assert!(val.get("entries").is_some());

    // 4. Test /stats/cb
    let response = app
        .oneshot(
            Request::builder()
                .uri("/stats/cb")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(val.get("circuit_breakers").is_some());
}

#[tokio::test]
async fn test_server_crawl_endpoint() {
    let app = create_test_app();

    let req_body = serde_json::json!({
        "start_url": "http://127.0.0.1:8080/internal",
        "max_depth": 1,
        "max_pages": 2
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/crawl")
                .header("content-type", "application/json")
                .body(Body::from(req_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["start_url"], "http://127.0.0.1:8080/internal");
    assert_eq!(val["pages_crawled"], 0);
}

#[tokio::test]
async fn test_batch_rank_rejects_ssrf_webhook() {
    let app = create_test_app();

    let req_body = serde_json::json!({
        "targets": [
            { "target": "example.com", "query": "seo tools" }
        ],
        "webhook_url": "http://127.0.0.1:8080/callback"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/rank/batch")
                .header("content-type", "application/json")
                .body(Body::from(req_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(val["error"]
        .as_str()
        .unwrap()
        .contains("Invalid webhook_url"));
}

#[tokio::test]
async fn test_csv_formatting_envelope() {
    use chrono::Utc;
    use frontlane_serp::core::format::render_csv;
    use frontlane_serp::core::types::{Envelope, Query, ResultItem, ResultType};

    let q = Query {
        text: "test query".to_string(),
        lang_code: "en".to_string(),
        region: "us".to_string(),
        ..Default::default()
    };
    let mut envelope = Envelope::new(
        &q,
        "req-123".to_string(),
        Utc::now(),
        vec!["google".to_string()],
    );
    envelope.results = vec![ResultItem {
        id: "res-1".to_string(),
        rank: 1,
        result_type: ResultType::Organic,
        title: "Test Title with, Comma".to_string(),
        url: "https://example.com/1".to_string(),
        display_url: "example.com/1".to_string(),
        snippet: "Test \"quoted\" description".to_string(),
        domain: "example.com".to_string(),
        favicon: "".to_string(),
        position: None,
        engine: "google".to_string(),
        domain_info: None,
        classification: None,
        extracted: None,
        score: None,
        engine_consensus: None,
        engines: None,
    }];

    let csv_output = render_csv(&envelope);
    assert!(csv_output.starts_with("rank,title,url,display_url,domain,engine,snippet,result_type"));
    assert!(csv_output.contains("1,\"Test Title with, Comma\",https://example.com/1,example.com/1,example.com,google,\"Test \"\"quoted\"\" description\",Organic"));
}

#[tokio::test]
async fn test_server_docs_and_openapi_endpoints() {
    let app = create_test_app();

    // 1. Test GET /docs returns Swagger UI
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("SwaggerUIBundle"));
    assert!(body_str.contains("/openapi.yaml"));

    // 2. Test GET /openapi.yaml returns OpenAPI spec
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/openapi.yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("openapi: 3.0.3"));
    assert!(body_str.contains("Frontlane SERP API"));

    // 3. Test GET / with Accept: text/html redirects to /docs
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header("accept", "text/html,application/xhtml+xml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap(),
        "/docs"
    );

    // 4. Test GET / with Accept: application/json includes docs_url
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("\"docs_url\":\"/docs\""));
    assert!(body_str.contains("\"openapi_url\":\"/openapi.yaml\""));
}
