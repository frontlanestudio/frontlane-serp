use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Path, Query as AxumQuery, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::compat::{
    convert_envelope_to_serpapi, convert_envelope_to_serper, SerpApiParams, SerperRequest,
};
use crate::core::error::SerpError;
use crate::core::format::{render_envelope, render_image_envelope};
use crate::core::response_builder::{enrich_image_result, enrich_result};
use crate::core::types::{
    Envelope, ImageEnvelope, OutputFormat, Pagination, Query, ResultItem, SerpFeature, API_VERSION,
    DEFAULT_QUERY_LIMIT,
};
use crate::jobs::types::BatchRankRequest;
use crate::rank::{probe_engine_rank, DeviceType, DomainMatchMode, RankRequest, RankStrategy};
use crate::server::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQueryParams {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub start: Option<usize>,
    #[serde(default)]
    pub filter: Option<bool>,
    #[serde(default)]
    pub features: Option<bool>,
    #[serde(default)]
    pub extract: Option<bool>,
    #[serde(default)]
    pub extract_top: Option<usize>,
    #[serde(default)]
    pub extract_mode: Option<String>,
    #[serde(default)]
    pub min_runes: Option<usize>,
    #[serde(default)]
    pub format: Option<String>,
    // Mega specific
    #[serde(default)]
    pub engines: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractQueryParams {
    pub url: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub min_runes: Option<usize>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub clean: Option<bool>,
    #[serde(default)]
    pub use_llms_txt: Option<bool>,
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractPostRequest {
    pub url: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub clean: Option<bool>,
    #[serde(default)]
    pub use_llms_txt: Option<bool>,
    #[serde(default)]
    pub min_runes: Option<usize>,
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchExtractRequest {
    pub urls: Vec<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub use_llms_txt: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractContactsQueryParams {
    pub url: Option<String>,
    #[serde(default)]
    pub crawl: Option<bool>,
    #[serde(default)]
    pub max_pages: Option<usize>,
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractContactsPostRequest {
    pub url: String,
    #[serde(default)]
    pub crawl: Option<bool>,
    #[serde(default)]
    pub max_pages: Option<usize>,
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchExtractItem {
    pub page_content: String,
    pub metadata: BatchExtractMetadata,
}

#[derive(Debug, Serialize)]
pub struct BatchExtractMetadata {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn build_query(params: &SearchQueryParams, headers: &HeaderMap) -> Query {
    let text = params
        .text
        .clone()
        .or_else(|| params.q.clone())
        .unwrap_or_default();

    let proxy_url = headers
        .get("x-proxy-url")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_country = headers
        .get("x-proxy-country")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_class = headers
        .get("x-proxy-class")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_provider = headers
        .get("x-proxy-provider")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_session_id = headers
        .get("x-proxy-session-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    Query {
        text,
        lang_code: params.lang.clone().unwrap_or_default(),
        region: params.region.clone().unwrap_or_default(),
        date_interval: params.date.clone().unwrap_or_default(),
        filetype: params.file.clone().unwrap_or_default(),
        site: params.site.clone().unwrap_or_default(),
        limit: params.limit.unwrap_or(DEFAULT_QUERY_LIMIT),
        start: params.start.unwrap_or(0),
        filter: params.filter.unwrap_or(true),
        features: params.features.unwrap_or(true),
        extract: params.extract.unwrap_or(false),
        extract_top: params.extract_top.unwrap_or(1),
        extract_mode: params
            .extract_mode
            .clone()
            .unwrap_or_else(|| "auto".to_string()),
        extract_min_runes: params.min_runes.unwrap_or(0),
        proxy_url,
        proxy_country,
        proxy_class,
        proxy_provider,
        proxy_session_id,
        proxy_override: None,
        insecure: true,
        guard_private_networks: false,
    }
}

fn determine_format(params_fmt: Option<&str>, accept_hdr: Option<&str>) -> OutputFormat {
    if let Some(f) = params_fmt {
        return OutputFormat::parse(f);
    }
    if let Some(acc) = accept_hdr {
        if acc.contains("text/csv") {
            return OutputFormat::Csv;
        } else if acc.contains("text/markdown") {
            return OutputFormat::Markdown;
        } else if acc.contains("text/plain") {
            return OutputFormat::Text;
        } else if acc.contains("application/x-ndjson") {
            return OutputFormat::Ndjson;
        }
    }
    OutputFormat::Json
}

fn format_response(env: &Envelope, format: OutputFormat) -> Response {
    match format {
        OutputFormat::Json => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string(env).unwrap_or_default(),
        )
            .into_response(),
        OutputFormat::Csv => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
            render_envelope(env, OutputFormat::Csv),
        )
            .into_response(),
        OutputFormat::Markdown => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            render_envelope(env, OutputFormat::Markdown),
        )
            .into_response(),
        OutputFormat::Text => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            render_envelope(env, OutputFormat::Text),
        )
            .into_response(),
        OutputFormat::Ndjson => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8")],
            render_envelope(env, OutputFormat::Ndjson),
        )
            .into_response(),
    }
}

fn format_image_response(env: &ImageEnvelope, format: OutputFormat) -> Response {
    match format {
        OutputFormat::Json => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string(env).unwrap_or_default(),
        )
            .into_response(),
        OutputFormat::Csv => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
            render_image_envelope(env, OutputFormat::Csv),
        )
            .into_response(),
        OutputFormat::Markdown => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            render_image_envelope(env, OutputFormat::Markdown),
        )
            .into_response(),
        OutputFormat::Text => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            render_image_envelope(env, OutputFormat::Text),
        )
            .into_response(),
        OutputFormat::Ndjson => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8")],
            render_image_envelope(env, OutputFormat::Ndjson),
        )
            .into_response(),
    }
}

fn error_response(err: SerpError) -> Response {
    let (status, msg) = match err {
        SerpError::EmptyResult => (StatusCode::NOT_FOUND, err.to_string()),
        SerpError::CaptchaDetected => (StatusCode::FORBIDDEN, err.to_string()),
        SerpError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, err.to_string()),
        SerpError::SearchTimeout => (StatusCode::GATEWAY_TIMEOUT, err.to_string()),
        SerpError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, err.to_string()),
        SerpError::CircuitBreakerOpen(_) => (StatusCode::SERVICE_UNAVAILABLE, err.to_string()),
        SerpError::Blocked(_) => (StatusCode::FORBIDDEN, err.to_string()),
        _ => (StatusCode::BAD_GATEWAY, err.to_string()),
    };

    (
        status,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({ "error": msg }).to_string(),
    )
        .into_response()
}

const OPENAPI_SPEC: &str = include_str!("../../docs/openapi.yaml");

pub async fn root_handler(headers: HeaderMap) -> Response {
    if headers
        .get(header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.contains("text/html"))
        .unwrap_or(false)
    {
        return axum::response::Redirect::temporary("/docs").into_response();
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "service": "frontlane-serp",
            "version": API_VERSION,
            "status": "online",
            "docs_url": "/docs",
            "openapi_url": "/openapi.yaml"
        })
        .to_string(),
    )
        .into_response()
}

pub async fn openapi_yaml_handler() -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/yaml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        OPENAPI_SPEC,
    )
        .into_response()
}

pub async fn swagger_ui_handler() -> Response {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Frontlane SERP - API Documentation</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5.18.2/swagger-ui.css" />
  <link rel="icon" type="image/svg+xml" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>🔍</text></svg>" />
  <style>
    body { margin: 0; padding: 0; background: #fafafa; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }
    .topbar { display: none !important; }
    .swagger-ui .info { margin: 24px 0; }
    .swagger-ui .info .title { font-size: 32px; color: #111827; }
    .swagger-ui .scheme-container { background: #fff; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }
  </style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5.18.2/swagger-ui-bundle.js"></script>
  <script src="https://unpkg.com/swagger-ui-dist@5.18.2/swagger-ui-standalone-preset.js"></script>
  <script>
    window.onload = () => {
      window.ui = SwaggerUIBundle({
        url: '/openapi.yaml',
        dom_id: '#swagger-ui',
        deepLinking: true,
        presets: [
          SwaggerUIBundle.presets.apis,
          SwaggerUIStandalonePreset
        ],
        layout: "BaseLayout"
      });
    };
  </script>
</body>
</html>"#;

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

pub async fn health_handler() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "status": "healthy",
            "version": API_VERSION,
            "timestamp": Utc::now().to_rfc3339()
        })
        .to_string(),
    )
        .into_response()
}

pub async fn ready_handler() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "ready": true,
            "version": API_VERSION
        })
        .to_string(),
    )
        .into_response()
}

pub async fn stats_handler(State(state): State<AppState>) -> Response {
    let lane_stats = state.lane_store.stats().await;
    let proxy_stats = state.proxy_manager.stats(Some(lane_stats)).await;
    let cb_stats = state.circuit_breaker_manager.all_stats().await;
    let captcha_metrics = crate::core::captcha::captcha_solver_metrics();

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "cache": {
                "entry_count": state.cache.len(),
            },
            "proxy": proxy_stats,
            "circuit_breakers": cb_stats,
            "captcha": captcha_metrics,
            "engines": state.engines.keys().cloned().collect::<Vec<_>>()
        })
        .to_string(),
    )
        .into_response()
}

pub async fn cache_stats_handler(State(state): State<AppState>) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "entry_count": state.cache.len(),
        })
        .to_string(),
    )
        .into_response()
}

pub async fn proxy_stats_handler(State(state): State<AppState>) -> Response {
    let lane_stats = state.lane_store.stats().await;
    let proxy_stats = state.proxy_manager.stats(Some(lane_stats)).await;

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!(proxy_stats).to_string(),
    )
        .into_response()
}

pub async fn circuit_breaker_stats_handler(State(state): State<AppState>) -> Response {
    let cb_stats = state.circuit_breaker_manager.all_stats().await;

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "circuit_breakers": cb_stats,
        })
        .to_string(),
    )
        .into_response()
}

pub async fn search_single_handler(
    Path(engine_name): Path<String>,
    headers: HeaderMap,
    AxumQuery(params): AxumQuery<SearchQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let normalized_engine = match engine_name.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        "hn" => "hackernews".to_string(),
        "gh" => "github".to_string(),
        "wiki" => "wikipedia".to_string(),
        n => n.to_string(),
    };

    let engine = match state.engines.get(&normalized_engine) {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": format!("Unsupported engine: {}", engine_name) }).to_string(),
            )
                .into_response();
        }
    };

    let query = build_query(&params, &headers);
    if query.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "Query text parameter ('text' or 'q') is required" }).to_string(),
        )
            .into_response();
    }

    let accept_header = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok());
    let format = determine_format(params.format.as_deref(), accept_header);

    // Cache lookup
    let cache_key = format!("{}:{}:{}", normalized_engine, query.text, query.start);
    if let Some(cached_json) = state.cache.get(&cache_key).await {
        if let Ok(cached_env) = serde_json::from_str::<Envelope>(&cached_json) {
            return format_response(&cached_env, format);
        }
    }

    let started_at = Utc::now();
    let start_time = Instant::now();
    let request_id = headers
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

    let mut effective_query = query.clone();
    if effective_query.proxy_url.is_none() {
        let country_hint =
            effective_query
                .proxy_country
                .as_deref()
                .or(if !effective_query.region.is_empty() {
                    Some(effective_query.region.as_str())
                } else {
                    None
                });
        effective_query.proxy_url = state
            .proxy_manager
            .resolve_proxy(Some(&normalized_engine), None, country_hint)
            .await;
    }

    let results = match engine.search(&effective_query).await {
        Ok(res) => {
            if let Some(ref p) = effective_query.proxy_url {
                state.proxy_manager.report_result(p, true).await;
            }
            res
        }
        Err(e) => {
            if let Some(ref p) = effective_query.proxy_url {
                if matches!(e, SerpError::CaptchaDetected | SerpError::Blocked(_)) {
                    state.proxy_manager.report_challenge(p).await;
                } else {
                    state.proxy_manager.report_result(p, false).await;
                }
            }
            return error_response(e);
        }
    };

    let took_ms = start_time.elapsed().as_millis() as i64;
    let mut env = Envelope::new(
        &effective_query,
        request_id,
        started_at,
        vec![normalized_engine.clone()],
    );
    env.meta.took_ms = took_ms;
    env.meta.engines_responded = vec![normalized_engine.clone()];

    let mut enriched_results: Vec<ResultItem> = Vec::new();
    let mut serp_features: Vec<SerpFeature> = Vec::new();

    for raw in results {
        for feat in &raw.features {
            serp_features.push(feat.clone());
        }
        let enriched = enrich_result(raw, &normalized_engine, effective_query.start);
        enriched_results.push(enriched);
    }

    if effective_query.extract {
        let extract_count = effective_query.extract_top.min(enriched_results.len());
        let lane_key = effective_query
            .proxy_session_id
            .as_ref()
            .map(|sid| crate::core::proxy::ProxyLaneKey::new("default", &normalized_engine, sid));
        for item in enriched_results.iter_mut().take(extract_count) {
            if let Ok(content) = state
                .extractor
                .extract_with_options(
                    &item.url,
                    true,
                    effective_query.proxy_url.as_deref(),
                    lane_key.as_ref(),
                )
                .await
            {
                item.extracted = Some(content);
            }
        }
    }

    env.results = enriched_results;
    env.serp_features = serp_features;
    env.pagination = Pagination {
        page: (query.start / 10) + 1,
        has_more: env.results.len() >= query.limit,
        next_start: query.start + query.limit,
    };

    if let Ok(json_str) = serde_json::to_string(&env) {
        state.cache.insert(cache_key, json_str).await;
    }

    format_response(&env, format)
}

pub async fn search_image_single_handler(
    Path(engine_name): Path<String>,
    headers: HeaderMap,
    AxumQuery(params): AxumQuery<SearchQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let normalized_engine = match engine_name.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        n => n.to_string(),
    };

    let engine = match state.engines.get(&normalized_engine) {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": format!("Unsupported engine: {}", engine_name) }).to_string(),
            )
                .into_response();
        }
    };

    let query = build_query(&params, &headers);
    if query.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "Query text parameter ('text' or 'q') is required" }).to_string(),
        )
            .into_response();
    }

    let accept_header = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok());
    let format = determine_format(params.format.as_deref(), accept_header);

    let started_at = Utc::now();
    let start_time = Instant::now();
    let request_id = headers
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

    let results = match engine.search_image(&query).await {
        Ok(res) => res,
        Err(e) => return error_response(e),
    };

    let took_ms = start_time.elapsed().as_millis() as i64;
    let mut image_results = Vec::new();
    for item in results {
        image_results.push(enrich_image_result(item, &normalized_engine));
    }

    let env = ImageEnvelope {
        query: crate::core::types::QueryEcho {
            text: query.text.clone(),
            lang: query.lang_code.clone(),
            region: query.region.clone(),
            engines_requested: vec![normalized_engine.clone()],
        },
        meta: crate::core::types::ResponseMeta {
            request_id,
            requested_at: started_at.to_rfc3339(),
            took_ms,
            engines_responded: vec![normalized_engine],
            engines_failed: Vec::new(),
            engine_errors: Vec::new(),
            version: API_VERSION.to_string(),
        },
        results: image_results,
        pagination: Pagination {
            page: 1,
            has_more: false,
            next_start: 0,
        },
    };

    format_image_response(&env, format)
}

pub async fn mega_search_handler(
    headers: HeaderMap,
    AxumQuery(params): AxumQuery<SearchQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let query = build_query(&params, &headers);
    if query.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "Query text parameter ('text' or 'q') is required" }).to_string(),
        )
            .into_response();
    }

    let engines_req = params
        .engines
        .as_deref()
        .map(|s| s.split(',').map(|e| e.trim().to_string()).collect())
        .unwrap_or_else(|| {
            vec![
                "google".to_string(),
                "bing".to_string(),
                "duckduckgo".to_string(),
            ]
        });

    let mode = params.mode.as_deref().unwrap_or("balanced");
    let accept_header = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok());
    let format = determine_format(params.format.as_deref(), accept_header);

    match state.mega.search(&query, &engines_req, mode).await {
        Ok(env) => format_response(&env, format),
        Err(e) => error_response(e),
    }
}

pub async fn mega_image_handler(
    headers: HeaderMap,
    AxumQuery(params): AxumQuery<SearchQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let query = build_query(&params, &headers);
    if query.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "Query text parameter ('text' or 'q') is required" }).to_string(),
        )
            .into_response();
    }

    let engines_req = params
        .engines
        .as_deref()
        .map(|s| s.split(',').map(|e| e.trim().to_string()).collect())
        .unwrap_or_else(|| vec!["google".to_string(), "bing".to_string()]);

    let accept_header = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok());
    let format = determine_format(params.format.as_deref(), accept_header);

    match state.mega.search_image(&query, &engines_req).await {
        Ok(env) => format_image_response(&env, format),
        Err(e) => error_response(e),
    }
}

pub async fn mega_engines_handler(State(state): State<AppState>) -> Response {
    let engines: Vec<String> = state.engines.keys().cloned().collect();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "engines": engines,
            "total": engines.len()
        })
        .to_string(),
    )
        .into_response()
}

pub async fn extract_handler(
    headers: HeaderMap,
    AxumQuery(params): AxumQuery<ExtractQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let url = match params.url {
        Some(u) if !u.trim().is_empty() => u,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": "Query parameter 'url' is required" }).to_string(),
            )
                .into_response();
        }
    };

    let proxy_url = headers
        .get("x-proxy-url")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_country = headers
        .get("x-proxy-country")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_session_id = headers
        .get("x-proxy-session-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let tenant = headers
        .get("x-tenant")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("default");

    let resolved_proxy = state
        .proxy_manager
        .resolve_proxy(None, proxy_url.as_deref(), proxy_country.as_deref())
        .await;

    let lane_key =
        proxy_session_id.map(|sid| crate::core::proxy::ProxyLaneKey::new(tenant, "extract", sid));

    let use_llms_txt = params.use_llms_txt.unwrap_or(false);
    match state
        .extractor
        .extract_with_options(
            &url,
            use_llms_txt,
            resolved_proxy.as_deref(),
            lane_key.as_ref(),
        )
        .await
    {
        Ok(content) => {
            let accept_header = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok());
            let format = determine_format(params.format.as_deref(), accept_header);
            match format {
                OutputFormat::Markdown | OutputFormat::Text => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                    content.content.unwrap_or_default(),
                )
                    .into_response(),
                _ => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                    serde_json::to_string(&content).unwrap_or_default(),
                )
                    .into_response(),
            }
        }
        Err(e) => error_response(e),
    }
}

pub async fn extract_post_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<ExtractPostRequest>,
) -> Response {
    if payload.url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "Field 'url' is required" }).to_string(),
        )
            .into_response();
    }

    let proxy_url = headers
        .get("x-proxy-url")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_country = headers
        .get("x-proxy-country")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_session_id = headers
        .get("x-proxy-session-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let tenant = headers
        .get("x-tenant")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("default");

    let resolved_proxy = state
        .proxy_manager
        .resolve_proxy(None, proxy_url.as_deref(), proxy_country.as_deref())
        .await;

    let lane_key =
        proxy_session_id.map(|sid| crate::core::proxy::ProxyLaneKey::new(tenant, "extract", sid));

    let use_llms_txt = payload.use_llms_txt.unwrap_or(false);
    match state
        .extractor
        .extract_with_options(
            &payload.url,
            use_llms_txt,
            resolved_proxy.as_deref(),
            lane_key.as_ref(),
        )
        .await
    {
        Ok(content) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string(&content).unwrap_or_default(),
        )
            .into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn extract_batch_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<BatchExtractRequest>,
) -> Response {
    if payload.urls.is_empty() || payload.urls.len() > 20 {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "URLs list must contain between 1 and 20 items" }).to_string(),
        )
            .into_response();
    }

    let proxy_url = headers
        .get("x-proxy-url")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_country = headers
        .get("x-proxy-country")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let proxy_session_id = headers
        .get("x-proxy-session-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let tenant = headers
        .get("x-tenant")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("default");

    let resolved_proxy = state
        .proxy_manager
        .resolve_proxy(None, proxy_url.as_deref(), proxy_country.as_deref())
        .await;

    let lane_key =
        proxy_session_id.map(|sid| crate::core::proxy::ProxyLaneKey::new(tenant, "extract", sid));

    let use_llms_txt = payload.use_llms_txt.unwrap_or(false);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(5));
    let mut join_set = tokio::task::JoinSet::new();

    for (idx, url) in payload.urls.into_iter().enumerate() {
        let sem = semaphore.clone();
        let extractor = state.extractor.clone();
        let proxy = resolved_proxy.clone();
        let key = lane_key.clone();

        join_set.spawn(async move {
            let _permit = sem.acquire().await.ok();
            let res = extractor
                .extract_with_options(&url, use_llms_txt, proxy.as_deref(), key.as_ref())
                .await;
            (idx, url, res)
        });
    }

    let mut indexed_items: Vec<(usize, BatchExtractItem)> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        if let Ok((idx, url, extract_res)) = res {
            let item = match extract_res {
                Ok(content) => BatchExtractItem {
                    page_content: content.content.unwrap_or_default(),
                    metadata: BatchExtractMetadata {
                        source: url,
                        title: content.title,
                        error: content.error,
                    },
                },
                Err(e) => BatchExtractItem {
                    page_content: String::new(),
                    metadata: BatchExtractMetadata {
                        source: url,
                        title: None,
                        error: Some(e.to_string()),
                    },
                },
            };
            indexed_items.push((idx, item));
        }
    }

    indexed_items.sort_by_key(|(idx, _)| *idx);
    let items: Vec<BatchExtractItem> = indexed_items.into_iter().map(|(_, item)| item).collect();

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::to_string(&items).unwrap_or_default(),
    )
        .into_response()
}

pub async fn crawl_post_handler(
    State(state): State<AppState>,
    Json(payload): Json<crate::crawl::CrawlOptions>,
) -> Response {
    match state.crawler.crawl(payload).await {
        Ok(result) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )
            .into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn extract_contacts_get_handler(
    headers: HeaderMap,
    AxumQuery(params): AxumQuery<ExtractContactsQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let url = match params.url {
        Some(ref u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": "Query parameter 'url' is required" }).to_string(),
            )
                .into_response();
        }
    };

    let crawl = params.crawl.unwrap_or(false);
    let max_pages = params.max_pages.unwrap_or(10);
    let max_depth = params.max_depth.unwrap_or(2);

    match crate::extract::scan_contacts(&state.http_client, &url, crawl, max_pages, max_depth).await
    {
        Ok(contacts) => {
            let accept_header = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok());
            let format = determine_format(params.format.as_deref(), accept_header);
            match format {
                OutputFormat::Markdown => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                    contacts.to_markdown(),
                )
                    .into_response(),
                OutputFormat::Csv => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                    contacts.to_csv(),
                )
                    .into_response(),
                OutputFormat::Text => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    contacts.to_console_text(),
                )
                    .into_response(),
                _ => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                    serde_json::to_string_pretty(&contacts).unwrap_or_default(),
                )
                    .into_response(),
            }
        }
        Err(e) => error_response(e),
    }
}

pub async fn extract_contacts_post_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<ExtractContactsPostRequest>,
) -> Response {
    let url = payload.url.trim().to_string();
    if url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "Field 'url' is required" }).to_string(),
        )
            .into_response();
    }

    let crawl = payload.crawl.unwrap_or(false);
    let max_pages = payload.max_pages.unwrap_or(10);
    let max_depth = payload.max_depth.unwrap_or(2);

    match crate::extract::scan_contacts(&state.http_client, &url, crawl, max_pages, max_depth).await
    {
        Ok(contacts) => {
            let accept_header = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok());
            let format = determine_format(payload.format.as_deref(), accept_header);
            match format {
                OutputFormat::Markdown => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                    contacts.to_markdown(),
                )
                    .into_response(),
                OutputFormat::Csv => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                    contacts.to_csv(),
                )
                    .into_response(),
                OutputFormat::Text => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    contacts.to_console_text(),
                )
                    .into_response(),
                _ => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                    serde_json::to_string_pretty(&contacts).unwrap_or_default(),
                )
                    .into_response(),
            }
        }
        Err(e) => error_response(e),
    }
}

pub async fn parse_google_handler(body: String) -> Response {
    let parsed = match crate::engines::google::parser::parse_html(&body, 0) {
        Ok(res) => res,
        Err(e) => return error_response(e),
    };
    let mut env = Envelope::new(
        &Query {
            text: String::new(),
            lang_code: String::new(),
            region: String::new(),
            date_interval: String::new(),
            filetype: String::new(),
            site: String::new(),
            limit: 10,
            start: 0,
            filter: false,
            features: true,
            extract: false,
            extract_top: 1,
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
        },
        uuid::Uuid::now_v7().to_string(),
        Utc::now(),
        vec!["google".to_string()],
    );

    for raw in parsed {
        for f in &raw.features {
            env.serp_features.push(f.clone());
        }
        env.results.push(enrich_result(raw, "google", 0));
    }

    format_response(&env, OutputFormat::Json)
}

pub async fn parse_bing_handler(body: String) -> Response {
    let parsed = match crate::engines::bing::parser::parse_html(&body, 0) {
        Ok(res) => res,
        Err(e) => return error_response(e),
    };
    let mut env = Envelope::new(
        &Query {
            text: String::new(),
            lang_code: String::new(),
            region: String::new(),
            date_interval: String::new(),
            filetype: String::new(),
            site: String::new(),
            limit: 10,
            start: 0,
            filter: false,
            features: true,
            extract: false,
            extract_top: 1,
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
        },
        uuid::Uuid::now_v7().to_string(),
        Utc::now(),
        vec!["bing".to_string()],
    );

    for raw in parsed {
        for f in &raw.features {
            env.serp_features.push(f.clone());
        }
        env.results.push(enrich_result(raw, "bing", 0));
    }

    format_response(&env, OutputFormat::Json)
}

// Serper search handler
pub async fn serper_search_handler(
    State(state): State<AppState>,
    Json(req): Json<SerperRequest>,
) -> Response {
    let engine = match state.engines.get("google") {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": "Google engine not initialized" }).to_string(),
            )
                .into_response();
        }
    };

    let num = if req.num == 0 { 10 } else { req.num };
    let page = if req.page == 0 { 1 } else { req.page };
    let start = (page - 1) * num;

    let query = Query {
        text: req.q.clone(),
        lang_code: req.hl.clone(),
        region: req.gl.clone(),
        date_interval: String::new(),
        filetype: String::new(),
        site: String::new(),
        limit: num,
        start,
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

    let started_at = Utc::now();
    let start_time = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let results = match engine.search(&query).await {
        Ok(res) => res,
        Err(e) => return error_response(e),
    };

    let took_ms = start_time.elapsed().as_millis() as i64;
    let mut env = Envelope::new(&query, request_id, started_at, vec!["google".to_string()]);
    env.meta.took_ms = took_ms;
    env.meta.engines_responded = vec!["google".to_string()];

    for raw in results {
        for feat in &raw.features {
            env.serp_features.push(feat.clone());
        }
        env.results.push(enrich_result(raw, "google", start));
    }

    let serper_res = convert_envelope_to_serper(&env, &req);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::to_string(&serper_res).unwrap_or_default(),
    )
        .into_response()
}

// SerpApi search handler
pub async fn serpapi_search_handler(
    State(state): State<AppState>,
    AxumQuery(params): AxumQuery<SerpApiParams>,
) -> Response {
    let norm_engine = match params.engine.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        n => n.to_string(),
    };

    let engine = match state.engines.get(&norm_engine) {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": format!("Unsupported engine: {}", params.engine) }).to_string(),
            )
                .into_response();
        }
    };

    let query = Query {
        text: params.q.clone(),
        lang_code: params.hl.clone(),
        region: params.gl.clone(),
        date_interval: String::new(),
        filetype: String::new(),
        site: String::new(),
        limit: params.num,
        start: params.start,
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

    let started_at = Utc::now();
    let start_time = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let results = match engine.search(&query).await {
        Ok(res) => res,
        Err(e) => return error_response(e),
    };

    let took_ms = start_time.elapsed().as_millis() as i64;
    let mut env = Envelope::new(&query, request_id, started_at, vec![norm_engine.clone()]);
    env.meta.took_ms = took_ms;
    env.meta.engines_responded = vec![norm_engine.clone()];

    for raw in results {
        for feat in &raw.features {
            env.serp_features.push(feat.clone());
        }
        env.results
            .push(enrich_result(raw, &norm_engine, query.start));
    }

    let serpapi_res = convert_envelope_to_serpapi(&env, &params);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::to_string(&serpapi_res).unwrap_or_default(),
    )
        .into_response()
}

// Rank query params
#[derive(Debug, Deserialize)]
pub struct RankQueryParams {
    pub target: Option<String>,
    pub q: Option<String>,
    pub query: Option<String>,
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub last_rank: Option<usize>,
    #[serde(default)]
    pub pagination_limit: Option<usize>,
    #[serde(default)]
    pub smart_full_fallback: Option<bool>,
    #[serde(default)]
    pub r#match: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
}

pub async fn rank_get_handler(
    Path(engine_name): Path<String>,
    AxumQuery(params): AxumQuery<RankQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let target = params.target.unwrap_or_default();
    let q_text = params.q.or(params.query).unwrap_or_default();

    if target.trim().is_empty() || q_text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "'target' and 'q' parameters are required" }).to_string(),
        )
            .into_response();
    }

    let strategy = match params.strategy.as_deref() {
        Some("basic") => RankStrategy::Basic,
        Some("custom") => RankStrategy::Custom,
        _ => RankStrategy::Smart,
    };

    let match_mode = match params.r#match.as_deref() {
        Some("exact") => DomainMatchMode::Exact,
        Some("wildcard") => DomainMatchMode::Wildcard,
        _ => DomainMatchMode::Subdomain,
    };

    let device = match params.device.as_deref() {
        Some("mobile") => DeviceType::Mobile,
        _ => DeviceType::Desktop,
    };

    let req = RankRequest {
        target,
        q: q_text,
        strategy,
        last_rank: params.last_rank.unwrap_or(0),
        pagination_limit: params.pagination_limit.unwrap_or(5),
        smart_full_fallback: params.smart_full_fallback.unwrap_or(false),
        r#match: match_mode,
        device,
        region: params.region.unwrap_or_default(),
        lang: params.lang.unwrap_or_default(),
    };

    execute_rank_probe(engine_name, req, state).await
}

pub async fn rank_post_handler(
    Path(engine_name): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<RankRequest>,
) -> Response {
    if req.target.trim().is_empty() || req.q.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "'target' and 'q' fields are required" }).to_string(),
        )
            .into_response();
    }
    execute_rank_probe(engine_name, req, state).await
}

async fn execute_rank_probe(engine_name: String, req: RankRequest, state: AppState) -> Response {
    let norm_engine = match engine_name.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        n => n.to_string(),
    };

    let engine = match state.engines.get(&norm_engine) {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": format!("Unsupported engine: {}", engine_name) }).to_string(),
            )
                .into_response();
        }
    };

    match probe_engine_rank(engine, &req).await {
        Ok(resp) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string(&resp).unwrap_or_default(),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": format!("Rank probe failed: {}", e) }).to_string(),
        )
            .into_response(),
    }
}

// Suggest handler
#[derive(Debug, Deserialize)]
pub struct SuggestQueryParams {
    pub q: Option<String>,
    pub query: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

pub async fn suggest_handler(
    Path(engine_name): Path<String>,
    AxumQuery(params): AxumQuery<SuggestQueryParams>,
    State(state): State<AppState>,
) -> Response {
    let q_text = params.q.or(params.query).unwrap_or_default();
    if q_text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "Query parameter ('q') is required" }).to_string(),
        )
            .into_response();
    }

    let lang = params.lang.as_deref().unwrap_or("en");
    let region = params.region.as_deref().unwrap_or("us");

    match state
        .suggest
        .suggest(&engine_name, &q_text, lang, region)
        .await
    {
        Ok(resp) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string(&resp).unwrap_or_default(),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": format!("Suggestion failed: {}", e) }).to_string(),
        )
            .into_response(),
    }
}

// Batch rank job handler
pub async fn batch_rank_handler(
    State(state): State<AppState>,
    Json(req): Json<BatchRankRequest>,
) -> Response {
    if req.targets.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": "At least one target item is required" }).to_string(),
        )
            .into_response();
    }

    if let Some(ref url) = req.webhook_url {
        if let Err(e) = crate::core::network_guard::validate_public_url(url).await {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": format!("Invalid webhook_url: {}", e) }).to_string(),
            )
                .into_response();
        }
    }

    let norm_engine = match req.engine.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        n => n.to_string(),
    };

    let engine = match state.engines.get(&norm_engine) {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "error": format!("Unsupported engine: {}", req.engine) }).to_string(),
            )
                .into_response();
        }
    };

    let total = req.targets.len();
    let job_id = state.jobs.submit_batch_rank(req, engine).await;

    (
        StatusCode::ACCEPTED,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "job_id": job_id,
            "status": "queued",
            "total": total
        })
        .to_string(),
    )
        .into_response()
}

// Job status handler
pub async fn job_status_handler(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    match state.jobs.get_job(&job_id).await {
        Some(details) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string(&details).unwrap_or_default(),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": format!("Job not found: {}", job_id) }).to_string(),
        )
            .into_response(),
    }
}
