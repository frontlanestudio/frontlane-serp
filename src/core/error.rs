use thiserror::Error;

#[derive(Error, Debug)]
pub enum SerpError {
    #[error("captcha detected")]
    CaptchaDetected,

    #[error("timeout. Cannot find element on page")]
    SearchTimeout,

    #[error("parser failure: {0}")]
    ParserFailure(String),

    #[error("engine internal error: {0}")]
    EngineInternal(String),

    #[error("proxy_connect: {0}")]
    ProxyConnect(String),

    #[error("proxy_auth: {0}")]
    ProxyAuth(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("empty_result")]
    EmptyResult,

    #[error("blocked: {0}")]
    Blocked(String),

    #[error("rate_limited")]
    RateLimited,

    #[error("invalid parameter: {0}")]
    InvalidParam(String),

    #[error("invalid limit: {0}")]
    InvalidLimit(String),

    #[error("invalid start: {0}")]
    InvalidStart(String),

    #[error("engine not found: {0}")]
    EngineNotFound(String),

    #[error("engine not initialized: {0}")]
    EngineNotInitialized(String),

    #[error("all engines failed")]
    AllEnginesFailed,

    #[error("circuit breaker open for engine: {0}")]
    CircuitBreakerOpen(String),

    #[error("network request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("url parse failed: {0}")]
    Url(#[from] url::ParseError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("other error: {0}")]
    Other(String),
}

impl SerpError {
    pub fn is_proxy_network_error(&self) -> bool {
        matches!(
            self,
            SerpError::ProxyConnect(_) | SerpError::ProxyAuth(_) | SerpError::Timeout(_)
        )
    }

    pub fn is_retryable(&self) -> bool {
        !matches!(
            self,
            SerpError::CaptchaDetected
                | SerpError::InvalidParam(_)
                | SerpError::InvalidLimit(_)
                | SerpError::InvalidStart(_)
                | SerpError::EmptyResult
        )
    }
}

pub type Result<T> = std::result::Result<T, SerpError>;
