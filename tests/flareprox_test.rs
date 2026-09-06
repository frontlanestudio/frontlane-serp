use axum::extract::Request;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use frontlane_serp::config::AppConfig;
use frontlane_serp::core::http_client::HttpClient;
use frontlane_serp::flareprox::{
    is_flareprox_url, normalize_flareprox_url, FlareProxConfig, FlareProxError,
    FLAREPROX_WORKER_JS,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[test]
fn test_worker_script_properties() {
    assert!(FLAREPROX_WORKER_JS.contains("addEventListener('fetch'"));
    assert!(FLAREPROX_WORKER_JS.contains("X-Target-URL"));
    assert!(FLAREPROX_WORKER_JS.contains("createProxyRequest"));
    assert!(FLAREPROX_WORKER_JS.contains("createProxyResponse"));
    assert!(FLAREPROX_WORKER_JS.contains("generateRandomIP"));
    assert!(FLAREPROX_WORKER_JS.contains("Access-Control-Allow-Origin"));
}

#[test]
fn test_flareprox_url_detection() {
    assert!(is_flareprox_url("https://flareprox-123.my-sub.workers.dev"));
    assert!(is_flareprox_url("http://flareprox-456.workers.dev/"));
    assert!(is_flareprox_url("flareprox+https://proxy.example.com"));
    assert!(is_flareprox_url("https://flareprox.mycorp.internal"));

    assert!(!is_flareprox_url("socks5://127.0.0.1:1080"));
    assert!(!is_flareprox_url("http://127.0.0.1:8080"));
    assert!(!is_flareprox_url("https://residential.proxy-provider.com:8000"));
}

#[test]
fn test_normalize_flareprox_url() {
    assert_eq!(
        normalize_flareprox_url("flareprox+https://flare.workers.dev"),
        "https://flare.workers.dev"
    );
    assert_eq!(
        normalize_flareprox_url("flare.workers.dev"),
        "https://flare.workers.dev"
    );
    assert_eq!(
        normalize_flareprox_url("https://flare.workers.dev"),
        "https://flare.workers.dev"
    );
}

#[test]
fn test_flareprox_config_credentials_resolution() {
    let mut config = FlareProxConfig::default();
    assert_eq!(config.worker_prefix, "flareprox");
    assert!(!config.enabled);

    // Missing credentials test
    let res = config.to_client();
    assert!(matches!(res, Err(FlareProxError::MissingCredentials(_))));

    // With credentials
    config.api_token = Some("mock_token".to_string());
    config.account_id = Some("mock_account".to_string());
    let client = config.to_client().unwrap();
    assert_eq!(client.worker_prefix(), "flareprox");
    assert!(client.generate_worker_name().starts_with("flareprox-"));
}

#[test]
fn test_app_config_includes_flareprox() {
    let cfg = AppConfig::default();
    assert!(!cfg.flareprox.enabled);
    assert_eq!(cfg.flareprox.worker_prefix, "flareprox");
    assert!(cfg.flareprox.workers.is_empty());
}

#[tokio::test]
async fn test_http_client_flareprox_gateway_routing() {
    // Spin up a mock FlareProx worker server
    let app = Router::new().route(
        "/",
        get(|req: Request| async move {
            let target = req
                .headers()
                .get("x-target-url")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
                .to_string();

            if target.contains("ifconfig.me") {
                "104.28.19.42".into_response()
            } else if !target.is_empty() {
                format!("proxied response for {}", target).into_response()
            } else {
                (axum::http::StatusCode::BAD_REQUEST, "no target").into_response()
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let worker_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Configure HttpClient to route via mock FlareProx worker gateway
    let flareprox_proxy = format!("flareprox+{}", worker_url);
    let client = HttpClient::new(Some(&flareprox_proxy), true, 5).unwrap();

    assert_eq!(client.flareprox_gateway(), Some(worker_url.as_str()));

    // Test request forwarding through the gateway
    let target_dest = "https://ifconfig.me/ip";
    let (status, body) = client
        .fetch_raw_response(target_dest, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(status.as_u16(), 200);
    assert_eq!(body, "104.28.19.42");

    // Test arbitrary website fetch
    let web_dest = "https://example.com/test";
    let (status2, body2) = client
        .fetch_raw_response(web_dest, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(status2.as_u16(), 200);
    assert_eq!(body2, "proxied response for https://example.com/test");
}
