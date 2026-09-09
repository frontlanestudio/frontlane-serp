use crate::core::error::{Result, SerpError};
use crate::core::impersonate::ImpersonationPool;
use crate::core::locale::build_accept_language_header;
use crate::core::proxy::ProxyLaneKey;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, COOKIE, USER_AGENT,
};
use reqwest::{Client, Proxy};
use std::time::Duration;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";
pub const MACOS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";
pub const LINUX_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPlatform {
    Windows,
    MacOS,
    Linux,
}

impl BrowserPlatform {
    pub fn default_user_agent(&self) -> &'static str {
        match self {
            BrowserPlatform::Windows => DEFAULT_USER_AGENT,
            BrowserPlatform::MacOS => MACOS_USER_AGENT,
            BrowserPlatform::Linux => LINUX_USER_AGENT,
        }
    }

    pub fn sec_ch_ua_platform(&self) -> &'static str {
        match self {
            BrowserPlatform::Windows => "\"Windows\"",
            BrowserPlatform::MacOS => "\"macOS\"",
            BrowserPlatform::Linux => "\"Linux\"",
        }
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    default_ua: String,
    platform: BrowserPlatform,
    flareprox_gateway: Option<String>,
    /// When set, direct (non-flareprox) fetches are routed through a
    /// browser-impersonating TLS/HTTP2 client from this pool instead of
    /// `client` above. See `crate::core::impersonate` for why this exists as
    /// an opt-in overlay rather than the default transport for every caller
    /// of `HttpClient`, and why it's a pool of profiles rather than one.
    impersonating: Option<ImpersonationPool>,
}

impl HttpClient {
    pub fn new(proxy_url: Option<&str>, insecure: bool, timeout_secs: u64) -> Result<Self> {
        Self::with_platform(proxy_url, insecure, timeout_secs, BrowserPlatform::Windows)
    }

    pub fn with_platform(
        proxy_url: Option<&str>,
        insecure: bool,
        timeout_secs: u64,
        platform: BrowserPlatform,
    ) -> Result<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(timeout_secs.max(5)))
            .danger_accept_invalid_certs(insecure)
            .gzip(true)
            .brotli(true);

        let mut flareprox_gateway = None;
        if let Some(proxy) = proxy_url {
            let p = proxy.trim();
            if !p.is_empty() {
                if crate::flareprox::is_flareprox_url(p) {
                    flareprox_gateway = Some(crate::flareprox::normalize_flareprox_url(p));
                } else {
                    let req_proxy = Proxy::all(p).map_err(|e| {
                        SerpError::ProxyConnect(format!("invalid proxy URL: {}", e))
                    })?;
                    builder = builder.proxy(req_proxy);
                }
            }
        }

        let default_ua = platform.default_user_agent().to_string();

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&default_ua)
                .unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_USER_AGENT)),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
        );
        headers.insert(
            "sec-ch-ua",
            HeaderValue::from_static(
                "\"Not(A:Brand\";v=\"99\", \"Google Chrome\";v=\"133\", \"Chromium\";v=\"133\"",
            ),
        );
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert(
            "sec-ch-ua-platform",
            HeaderValue::from_static(platform.sec_ch_ua_platform()),
        );
        headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("none"));
        headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
        headers.insert("upgrade-insecure-requests", HeaderValue::from_static("1"));

        builder = builder.default_headers(headers);

        let client = builder.build()?;
        Ok(Self {
            client,
            default_ua,
            platform,
            flareprox_gateway,
            impersonating: None,
        })
    }

    pub fn with_flareprox_gateway(mut self, gateway_url: Option<String>) -> Self {
        self.flareprox_gateway = gateway_url.map(|u| crate::flareprox::normalize_flareprox_url(&u));
        self
    }

    pub fn flareprox_gateway(&self) -> Option<&str> {
        self.flareprox_gateway.as_deref()
    }

    /// Enable browser TLS/HTTP2 fingerprint impersonation for direct (non-
    /// flareprox) fetches made by this client. `proxy_url`/`insecure`/
    /// `timeout_secs` mirror the constructor args since each client in the
    /// pool has its own connection pool and can't share `reqwest`'s.
    pub fn with_impersonation(
        mut self,
        enabled: bool,
        proxy_url: Option<&str>,
        insecure: bool,
        timeout_secs: u64,
    ) -> Result<Self> {
        self.impersonating = if enabled {
            Some(ImpersonationPool::new(proxy_url, insecure, timeout_secs)?)
        } else {
            None
        };
        Ok(self)
    }

    /// Build a scraping client with optional browser impersonation, falling back
    /// to a plain client if impersonation fails to initialize.
    pub fn for_scraping(
        &self,
        enabled: bool,
        proxy_url: Option<&str>,
        insecure: bool,
        timeout_secs: u64,
    ) -> Self {
        self.clone()
            .with_impersonation(enabled, proxy_url, insecure, timeout_secs)
            .unwrap_or_else(|e| {
                tracing::warn!(
                    error = %e,
                    "failed to initialize browser impersonation client, falling back to plain HTTP client"
                );
                self.clone()
            })
    }

    pub fn is_impersonating(&self) -> bool {
        self.impersonating.is_some()
    }

    pub fn inner(&self) -> &Client {
        &self.client
    }

    pub fn user_agent(&self) -> &str {
        &self.default_ua
    }

    pub fn platform(&self) -> BrowserPlatform {
        self.platform
    }

    pub async fn get(&self, url: &str) -> reqwest::Result<reqwest::Response> {
        if let Some(ref gateway) = self.flareprox_gateway {
            self.client
                .get(gateway)
                .header("X-Target-URL", url)
                .send()
                .await
        } else {
            self.client.get(url).send().await
        }
    }

    pub async fn post_json<T: serde::Serialize>(
        &self,
        url: &str,
        body: &T,
    ) -> reqwest::Result<reqwest::Response> {
        if let Some(ref gateway) = self.flareprox_gateway {
            self.client
                .post(gateway)
                .header("X-Target-URL", url)
                .json(body)
                .send()
                .await
        } else {
            self.client.post(url).json(body).send().await
        }
    }

    pub async fn fetch(&self, url: &str, lang: Option<&str>) -> Result<String> {
        self.fetch_with_options(url, lang, None, None, None).await
    }

    pub async fn fetch_with_options(
        &self,
        url: &str,
        lang: Option<&str>,
        custom_ua: Option<&str>,
        cookies: Option<&str>,
        extra_headers: Option<HeaderMap>,
    ) -> Result<String> {
        let (status, body) = self
            .fetch_raw_response(url, lang, custom_ua, cookies, extra_headers)
            .await?;

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SerpError::RateLimited);
        }
        if status.as_u16() == 403 || status.as_u16() == 407 {
            return Err(SerpError::Blocked(format!("HTTP status {}", status)));
        }

        Ok(body)
    }

    pub async fn fetch_with_headers(&self, url: &str, headers: HeaderMap) -> Result<String> {
        self.fetch_with_options(url, None, None, None, Some(headers))
            .await
    }

    /// Build the set of extra headers (UA override, cookies, Accept-Language,
    /// caller-supplied headers) shared by both the direct `reqwest` fetch path
    /// and the impersonating `wreq` fetch path.
    fn build_extra_headers(
        &self,
        lang: Option<&str>,
        custom_ua: Option<&str>,
        cookies: Option<&str>,
        extra_headers: Option<HeaderMap>,
    ) -> HeaderMap {
        let mut headers = HeaderMap::new();

        if let Some(ua) = custom_ua {
            if !ua.trim().is_empty() {
                if let Ok(val) = HeaderValue::from_str(ua) {
                    headers.insert(USER_AGENT, val);
                }
            }
        }

        if let Some(cookie_str) = cookies {
            if !cookie_str.trim().is_empty() {
                if let Ok(val) = HeaderValue::from_str(cookie_str) {
                    headers.insert(COOKIE, val);
                }
            }
        }

        if let Some(l) = lang {
            let accept_lang = build_accept_language_header(l);
            if !accept_lang.is_empty() {
                if let Ok(val) = HeaderValue::from_str(&accept_lang) {
                    headers.insert(ACCEPT_LANGUAGE, val);
                }
            }
        }

        if let Some(extra) = extra_headers {
            for (name, value) in extra.iter() {
                if let Ok(hname) = HeaderName::from_bytes(name.as_ref()) {
                    headers.insert(hname, value.clone());
                }
            }
        }

        headers
    }

    pub async fn fetch_raw_response(
        &self,
        url: &str,
        lang: Option<&str>,
        custom_ua: Option<&str>,
        cookies: Option<&str>,
        extra_headers: Option<HeaderMap>,
    ) -> Result<(reqwest::StatusCode, String)> {
        self.fetch_raw_response_with_lane(url, lang, custom_ua, cookies, extra_headers, None)
            .await
    }

    /// Same as [`Self::fetch_raw_response`], but takes a lane key so that,
    /// when impersonation is enabled, the browser/OS profile used for this
    /// request is picked consistently for the lane rather than a fixed
    /// default. See `crate::core::impersonate::ImpersonationPool` for why.
    pub async fn fetch_raw_response_with_lane(
        &self,
        url: &str,
        lang: Option<&str>,
        custom_ua: Option<&str>,
        cookies: Option<&str>,
        extra_headers: Option<HeaderMap>,
        lane_key: Option<&ProxyLaneKey>,
    ) -> Result<(reqwest::StatusCode, String)> {
        let headers = self.build_extra_headers(lang, custom_ua, cookies, extra_headers);

        // The flareprox gateway is our own trusted Cloudflare Worker: our TLS
        // handshake terminates there, and it performs the actual outbound
        // fetch to `url` on our behalf, so impersonating a browser fingerprint
        // on this leg of the connection would have no effect on the leg that
        // matters. Route it through the plain client either way.
        if let Some(ref gateway) = self.flareprox_gateway {
            let mut req = self.client.get(gateway).header("X-Target-URL", url);
            for (name, value) in headers.iter() {
                req = req.header(name, value.clone());
            }
            let resp = req.send().await?;
            let status = resp.status();
            let body = resp.text().await?;
            return Ok((status, body));
        }

        if let Some(ref pool) = self.impersonating {
            let impersonating = pool.client_for(lane_key);
            match impersonating.fetch_raw_response(url, &headers).await {
                Ok((status_u16, body)) => {
                    let status = reqwest::StatusCode::from_u16(status_u16)
                        .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
                    return Ok((status, body));
                }
                Err(e) => {
                    // The impersonating client's BoringSSL stack can fail to
                    // even complete a handshake in some network environments
                    // (a strict egress proxy that's fine with rustls but
                    // resets on the ClientHello a browser-emulating library
                    // sends, for example) where the plain client works fine.
                    // Fall back rather than hard-failing a request over what
                    // is meant to be a hardening feature, not a requirement.
                    tracing::warn!(
                        error = %e,
                        url,
                        profile = impersonating.profile_label(),
                        "browser impersonation request failed, falling back to plain HTTP client"
                    );
                }
            }
        }

        let mut req = self.client.get(url);
        for (name, value) in headers.iter() {
            req = req.header(name, value.clone());
        }
        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        Ok((status, body))
    }

    pub async fn fetch_raw_response_with_proxy(
        &self,
        url: &str,
        proxy_override: Option<&str>,
        lang: Option<&str>,
        custom_ua: Option<&str>,
        cookies: Option<&str>,
        extra_headers: Option<HeaderMap>,
    ) -> Result<(reqwest::StatusCode, String)> {
        self.fetch_raw_response_with_proxy_and_lane(
            url,
            proxy_override,
            lang,
            custom_ua,
            cookies,
            extra_headers,
            None,
        )
        .await
    }

    /// Same as [`Self::fetch_raw_response_with_proxy`], but takes a lane key
    /// (see [`Self::fetch_raw_response_with_lane`]).
    #[allow(clippy::too_many_arguments)]
    pub async fn fetch_raw_response_with_proxy_and_lane(
        &self,
        url: &str,
        proxy_override: Option<&str>,
        lang: Option<&str>,
        custom_ua: Option<&str>,
        cookies: Option<&str>,
        extra_headers: Option<HeaderMap>,
        lane_key: Option<&ProxyLaneKey>,
    ) -> Result<(reqwest::StatusCode, String)> {
        if let Some(p) = proxy_override {
            let clean = p.trim();
            if !clean.is_empty() && crate::flareprox::is_flareprox_url(clean) {
                let gateway = crate::flareprox::normalize_flareprox_url(clean);
                let mut req = self.client.get(&gateway).header("X-Target-URL", url);

                if let Some(ua) = custom_ua.map(str::trim).filter(|s| !s.is_empty()) {
                    if let Ok(val) = HeaderValue::from_str(ua) {
                        req = req.header(USER_AGENT, val);
                    }
                }

                if let Some(cookie_str) = cookies.map(str::trim).filter(|s| !s.is_empty()) {
                    if let Ok(val) = HeaderValue::from_str(cookie_str) {
                        req = req.header(COOKIE, val);
                    }
                }

                if let Some(l) = lang {
                    let accept_lang = build_accept_language_header(l);
                    if !accept_lang.is_empty() {
                        req = req.header(ACCEPT_LANGUAGE, accept_lang);
                    }
                }

                if let Some(headers) = extra_headers {
                    for (name, value) in headers.iter() {
                        if let Ok(hname) = HeaderName::from_bytes(name.as_ref()) {
                            req = req.header(hname, value.clone());
                        }
                    }
                }

                let resp = req.send().await?;
                let status = resp.status();
                let body = resp.text().await?;
                return Ok((status, body));
            }
        }

        self.fetch_raw_response_with_lane(url, lang, custom_ua, cookies, extra_headers, lane_key)
            .await
    }
}
