use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use frontlane_serp::config::AppConfig;
use frontlane_serp::core::http_client::HttpClient;
use frontlane_serp::server::create_router;
use frontlane_serp::server::state::AppState;

#[tokio::test]
async fn test_server_routes_health_and_root() {
    let config = AppConfig::default();
    let http_client = HttpClient::new(None, true, 10).expect("http client");
    let state = AppState::new(config, vec![], http_client);
    let app = create_router(state);

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
    let config = AppConfig::default();
    let http_client = HttpClient::new(None, true, 10).expect("http client");
    let state = AppState::new(config, vec![], http_client);
    let app = create_router(state);

    let sample_html = std::fs::read_to_string("google/testdata/search_results.html")
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
    let config = AppConfig::default();
    let http_client = HttpClient::new(None, true, 10).expect("http client");
    let state = AppState::new(config, vec![], http_client);
    let app = create_router(state);

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
    let config = AppConfig::default();
    let http_client = HttpClient::new(None, true, 10).expect("http client");
    let state = AppState::new(config, vec![], http_client);
    let app = create_router(state);

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
    let config = AppConfig::default();
    let http_client = HttpClient::new(None, true, 10).expect("http client");
    let state = AppState::new(config, vec![], http_client);
    let app = create_router(state);

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
