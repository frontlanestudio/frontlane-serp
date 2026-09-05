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
