use axum::http::HeaderValue;
use axum::{extract::Request, middleware::Next, response::Response};
use tracing::info;
use uuid::Uuid;

pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let request_id = match req.headers().get("x-request-id") {
        Some(v) => v.to_str().unwrap_or_default().to_string(),
        None => Uuid::now_v7().to_string(),
    };

    let start = std::time::Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();

    // Insert request ID into extensions or headers if needed
    req.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap_or(HeaderValue::from_static("")),
    );

    let mut response = next.run(req).await;

    let elapsed = start.elapsed();
    info!(
        method = %method,
        uri = %uri,
        status = %response.status(),
        latency_ms = elapsed.as_millis(),
        request_id = %request_id,
        "handled request"
    );

    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap_or(HeaderValue::from_static("")),
    );

    response
}
