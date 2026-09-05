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

use crate::core::error::SerpError;
use crate::core::format::{render_envelope, render_image_envelope};
use crate::core::response_builder::{enrich_image_result, enrich_result};
use crate::core::types::{
    DEFAULT_QUERY_LIMIT, Envelope, ImageEnvelope, OutputFormat, Pagination, Query, ResultItem,
    SerpFeature, API_VERSION,
};
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
        extract_mode: params.extract_mode.clone().unwrap_or_else(|| "auto".to_string()),
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
        if acc.contains("text/markdown") {
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

pub async fn root_handler() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "service": "frontlane-serp",
            "version": API_VERSION,
            "status": "online"
        })
        .to_string(),
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
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({
            "cache": {
                "entry_count": state.cache.len(),
            },
            "engines": state.engines.keys().cloned().collect::<Vec<_>>()
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

    let results = match engine.search(&query).await {
        Ok(res) => res,
        Err(e) => return error_response(e),
    };

    let took_ms = start_time.elapsed().as_millis() as i64;
    let mut env = Envelope::new(&query, request_id, started_at, vec![normalized_engine.clone()]);
    env.meta.took_ms = took_ms;
    env.meta.engines_responded = vec![normalized_engine.clone()];

    let mut enriched_results: Vec<ResultItem> = Vec::new();
    let mut serp_features: Vec<SerpFeature> = Vec::new();

    for raw in results {
        for feat in &raw.features {
            serp_features.push(feat.clone());
        }
        let enriched = enrich_result(raw, &normalized_engine, query.start);
        enriched_results.push(enriched);
    }

    if query.extract {
        let extract_count = query.extract_top.min(enriched_results.len());
        for item in enriched_results.iter_mut().take(extract_count) {
            if let Ok(content) = state.extractor.extract(&item.url, true).await {
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
        .unwrap_or_else(|| vec!["google".to_string(), "bing".to_string(), "duckduckgo".to_string()]);

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

    let use_llms_txt = params.use_llms_txt.unwrap_or(false);
    match state.extractor.extract(&url, use_llms_txt).await {
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

    let use_llms_txt = payload.use_llms_txt.unwrap_or(false);
    match state.extractor.extract(&payload.url, use_llms_txt).await {
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

    let use_llms_txt = payload.use_llms_txt.unwrap_or(false);
    let mut items = Vec::new();

    for url in &payload.urls {
        match state.extractor.extract(url, use_llms_txt).await {
            Ok(content) => {
                items.push(BatchExtractItem {
                    page_content: content.content.unwrap_or_default(),
                    metadata: BatchExtractMetadata {
                        source: url.clone(),
                        title: content.title,
                        error: content.error,
                    },
                });
            }
            Err(e) => {
                items.push(BatchExtractItem {
                    page_content: String::new(),
                    metadata: BatchExtractMetadata {
                        source: url.clone(),
                        title: None,
                        error: Some(e.to_string()),
                    },
                });
            }
        }
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::to_string(&items).unwrap_or_default(),
    )
        .into_response()
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

