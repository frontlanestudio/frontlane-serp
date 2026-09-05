use crate::core::error::{Result, SerpError};
use crate::core::locale::build_accept_language_header;
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

        if let Some(proxy) = proxy_url {
            if !proxy.trim().is_empty() {
                let req_proxy = Proxy::all(proxy)
                    .map_err(|e| SerpError::ProxyConnect(format!("invalid proxy URL: {}", e)))?;
                builder = builder.proxy(req_proxy);
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
        })
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
        self.client.get(url).send().await
    }

    pub async fn post_json<T: serde::Serialize>(
        &self,
        url: &str,
        body: &T,
    ) -> reqwest::Result<reqwest::Response> {
        self.client.post(url).json(body).send().await
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

    pub async fn fetch_raw_response(
        &self,
        url: &str,
        lang: Option<&str>,
        custom_ua: Option<&str>,
        cookies: Option<&str>,
        extra_headers: Option<HeaderMap>,
    ) -> Result<(reqwest::StatusCode, String)> {
        let mut req = self.client.get(url);

        if let Some(ua) = custom_ua {
            if !ua.trim().is_empty() {
                if let Ok(val) = HeaderValue::from_str(ua) {
                    req = req.header(USER_AGENT, val);
                }
            }
        }

        if let Some(cookie_str) = cookies {
            if !cookie_str.trim().is_empty() {
                if let Ok(val) = HeaderValue::from_str(cookie_str) {
                    req = req.header(COOKIE, val);
                }
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
        Ok((status, body))
    }
}
