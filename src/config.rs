use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub app: GeneralAppConfig,
    #[serde(default)]
    pub extract: ExtractConfig,
    #[serde(default)]
    pub proxies: ProxiesConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub resilience: ResilienceConfig,
    #[serde(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    #[serde(default)]
    pub cors: CorsConfig,
    #[serde(default)]
    pub captcha: CaptchaConfig,
    #[serde(default)]
    pub flareprox: crate::flareprox::FlareProxConfig,
    #[serde(flatten)]
    pub engines: HashMap<String, EngineConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub raw_requests: bool,
    #[serde(default = "default_true")]
    pub insecure: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            debug: false,
            verbose: false,
            raw_requests: false,
            insecure: true,
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    7000
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralAppConfig {
    #[serde(default = "default_log_format")]
    pub log_format: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub browser_path: String,
    #[serde(default)]
    pub profiles: String,
    #[serde(default)]
    pub head: bool,
    #[serde(default)]
    pub leakless: bool,
    #[serde(default)]
    pub leave_head: bool,
    #[serde(default = "default_block_resources")]
    pub block_resources: String,
    #[serde(default = "default_true")]
    pub block_trackers: bool,
    #[serde(default = "default_max_processes")]
    pub max_processes: usize,
    #[serde(default = "default_idle_ttl")]
    pub idle_ttl: String,
    #[serde(default = "default_mega_timeout")]
    pub mega_timeout: String,
}

impl Default for GeneralAppConfig {
    fn default() -> Self {
        Self {
            log_format: default_log_format(),
            timeout: default_timeout(),
            browser_path: String::new(),
            profiles: String::new(),
            head: false,
            leakless: false,
            leave_head: false,
            block_resources: default_block_resources(),
            block_trackers: true,
            max_processes: default_max_processes(),
            idle_ttl: default_idle_ttl(),
            mega_timeout: default_mega_timeout(),
        }
    }
}

fn default_log_format() -> String {
    "text".to_string()
}

fn default_timeout() -> u64 {
    15
}

fn default_block_resources() -> String {
    "image,font,css,media".to_string()
}

fn default_max_processes() -> usize {
    6
}

fn default_idle_ttl() -> String {
    "5m".to_string()
}

fn default_mega_timeout() -> String {
    "90s".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default = "default_extract_timeout")]
    pub timeout: String,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_mode: default_mode(),
            timeout: default_extract_timeout(),
            max_bytes: default_max_bytes(),
            max_concurrent: default_max_concurrent(),
        }
    }
}

fn default_mode() -> String {
    "auto".to_string()
}

fn default_extract_timeout() -> String {
    "20s".to_string()
}

fn default_max_bytes() -> usize {
    2000000
}

fn default_max_concurrent() -> usize {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxiesConfig {
    #[serde(default = "default_true")]
    pub allow_request_proxy_url: bool,
    #[serde(default)]
    pub global: Option<String>,
    #[serde(default)]
    pub entries: Vec<ProxyEntry>,
    #[serde(default)]
    pub health: ProxyHealthConfig,
    #[serde(default)]
    pub lanes: ProxyLanesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyEntry {
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHealthConfig {
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
}

impl Default for ProxyHealthConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
        }
    }
}

fn default_failure_threshold() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLanesConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_lanes")]
    pub max_lanes: usize,
    #[serde(default = "default_true")]
    pub drop_cookies_on_challenge: bool,
}

impl Default for ProxyLanesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lanes: default_max_lanes(),
            drop_cookies_on_challenge: true,
        }
    }
}

fn default_max_lanes() -> usize {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_ttl")]
    pub ttl_seconds: u64,
    #[serde(default = "default_cache_size")]
    pub max_size: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: default_cache_ttl(),
            max_size: default_cache_size(),
        }
    }
}

fn default_cache_ttl() -> u64 {
    120
}

fn default_cache_size() -> u64 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub allow_endpoint_fallback: bool,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            allow_endpoint_fallback: false,
        }
    }
}

fn default_max_retries() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    #[serde(default = "default_cb_failures")]
    pub failures: u32,
    #[serde(default = "default_cb_recovery")]
    pub recovery_seconds: u64,
    #[serde(default = "default_cb_successes")]
    pub successes: u32,
}

fn default_cb_failures() -> u32 {
    5
}

fn default_cb_recovery() -> u64 {
    60
}

fn default_cb_successes() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_cors_origins")]
    pub allow_origins: String,
    #[serde(default = "default_cors_methods")]
    pub allow_methods: String,
    #[serde(default = "default_cors_headers")]
    pub allow_headers: String,
    #[serde(default = "default_cors_max_age")]
    pub max_age: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_origins: default_cors_origins(),
            allow_methods: default_cors_methods(),
            allow_headers: default_cors_headers(),
            max_age: default_cors_max_age(),
        }
    }
}

fn default_cors_origins() -> String {
    "*".to_string()
}

fn default_cors_methods() -> String {
    "GET, POST, OPTIONS".to_string()
}

fn default_cors_headers() -> String {
    "Origin, Content-Type, Accept, Authorization, X-Use-Proxy, X-Proxy-URL, X-Proxy-Country, X-Proxy-Class, X-Proxy-Provider, X-Proxy-Session-ID, X-Request-ID, X-Tenant, X-Use-Profile".to_string()
}

fn default_cors_max_age() -> u64 {
    86400
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaConfig {
    #[serde(default)]
    pub solver_enabled: bool,
    #[serde(default = "default_captcha_provider")]
    pub provider: String,
    #[serde(default)]
    pub apikey: Option<String>,
}

impl Default for CaptchaConfig {
    fn default() -> Self {
        Self {
            solver_enabled: false,
            provider: default_captcha_provider(),
            apikey: None,
        }
    }
}

fn default_captcha_provider() -> String {
    "capsolver".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    #[serde(default = "default_rate_requests")]
    pub rate_requests: u32,
    #[serde(default = "default_rate_burst")]
    pub rate_burst: u32,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub captcha: Option<bool>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            rate_requests: default_rate_requests(),
            rate_burst: default_rate_burst(),
            proxy: None,
            captcha: None,
        }
    }
}

fn default_rate_requests() -> u32 {
    60
}

fn default_rate_burst() -> u32 {
    3
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let p = path.as_ref();
        if p.exists() {
            let content = std::fs::read_to_string(p)?;
            let mut cfg: AppConfig = serde_yaml::from_str(&content)?;
            cfg.apply_env_overrides();
            Ok(cfg)
        } else {
            let mut cfg = AppConfig::default();
            cfg.apply_env_overrides();
            Ok(cfg)
        }
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(host) = std::env::var("OPENSERP_HOST").or_else(|_| std::env::var("HOST")) {
            self.server.host = host;
        }
        if let Ok(port_str) = std::env::var("OPENSERP_PORT").or_else(|_| std::env::var("PORT")) {
            if let Ok(port) = port_str.parse::<u16>() {
                self.server.port = port;
            }
        }
        if let Ok(proxy) = std::env::var("OPENSERP_PROXY").or_else(|_| std::env::var("HTTP_PROXY"))
        {
            self.proxies.global = Some(proxy);
        }
        if let Ok(debug) = std::env::var("OPENSERP_DEBUG") {
            self.server.debug = debug == "1" || debug.eq_ignore_ascii_case("true");
        }
        if let Ok(token) =
            std::env::var("CLOUDFLARE_API_TOKEN").or_else(|_| std::env::var("CF_API_TOKEN"))
        {
            self.flareprox.api_token = Some(token);
        }
        if let Ok(account) =
            std::env::var("CLOUDFLARE_ACCOUNT_ID").or_else(|_| std::env::var("CF_ACCOUNT_ID"))
        {
            self.flareprox.account_id = Some(account);
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut engines = HashMap::new();
        engines.insert("google".to_string(), EngineConfig::default());
        engines.insert("bing".to_string(), EngineConfig::default());
        engines.insert("duckduckgo".to_string(), EngineConfig::default());
        engines.insert("yandex".to_string(), EngineConfig::default());
        engines.insert("baidu".to_string(), EngineConfig::default());
        engines.insert("ecosia".to_string(), EngineConfig::default());

        Self {
            server: ServerConfig::default(),
            app: GeneralAppConfig::default(),
            extract: ExtractConfig::default(),
            proxies: ProxiesConfig::default(),
            cache: CacheConfig::default(),
            resilience: ResilienceConfig::default(),
            circuit_breaker: None,
            cors: CorsConfig::default(),
            captcha: CaptchaConfig::default(),
            flareprox: crate::flareprox::FlareProxConfig::default(),
            engines,
        }
    }
}
