pub mod handlers;
pub mod middleware;
pub mod state;

use axum::{
    middleware::from_fn,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::config::AppConfig;
use crate::core::engine::SearchEngine;
use crate::core::http_client::HttpClient;
use crate::server::handlers::*;
use crate::server::middleware::request_id_middleware;
use crate::server::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health & Stats
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/stats", get(stats_handler))
        .route("/stats/cache", get(stats_handler))
        // Mega
        .route("/mega/search", get(mega_search_handler))
        .route("/mega/image", get(mega_image_handler))
        .route("/mega/engines", get(mega_engines_handler))
        // Single engine
        .route("/{engine}/search", get(search_single_handler))
        .route("/{engine}/image", get(search_image_single_handler))
        // Raw HTML parsing
        .route("/google/parse", post(parse_google_handler))
        .route("/bing/parse", post(parse_bing_handler))
        // Extraction
        .route("/extract", get(extract_handler).post(extract_post_handler))
        .route("/extract/batch", post(extract_batch_handler))
        .layer(from_fn(request_id_middleware))
        .layer(cors)
        .with_state(state)
}

pub async fn run_server(
    config: AppConfig,
    engines: Vec<Arc<dyn SearchEngine>>,
    http_client: HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    let state = AppState::new(config, engines, http_client);
    let app = create_router(state);

    tracing::info!("Starting frontlane-serp server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
