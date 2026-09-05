use std::time::Duration;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use reqwest::{Client, Proxy};
use crate::core::error::{Result, SerpError};
use crate::core::locale::build_accept_language_header;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new(proxy_url: Option<&str>, insecure: bool, timeout_secs: u64) -> Result<Self> {
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

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(DEFAULT_USER_AGENT),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
        );
        headers.insert(
            "sec-ch-ua",
            HeaderValue::from_static("\"Not(A:Brand\";v=\"99\", \"Google Chrome\";v=\"133\", \"Chromium\";v=\"133\""),
        );
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static("\"Windows\""));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("none"));
        headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
        headers.insert("upgrade-insecure-requests", HeaderValue::from_static("1"));

        builder = builder.default_headers(headers);

        let client = builder.build()?;
        Ok(Self { client })
    }

    pub fn inner(&self) -> &Client {
        &self.client
    }

    pub async fn get(&self, url: &str) -> reqwest::Result<reqwest::Response> {
        self.client.get(url).send().await
    }

    pub async fn post_json<T: serde::Serialize>(&self, url: &str, body: &T) -> reqwest::Result<reqwest::Response> {
        self.client.post(url).json(body).send().await
    }

    pub async fn fetch(&self, url: &str, lang: Option<&str>) -> Result<String> {
        let mut req = self.client.get(url);
        if let Some(l) = lang {
            let accept_lang = build_accept_language_header(l);
            if !accept_lang.is_empty() {
                req = req.header(ACCEPT_LANGUAGE, accept_lang);
            }
        }

        let resp = req.send().await?;
        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SerpError::RateLimited);
        }
        if status.as_u16() == 403 || status.as_u16() == 407 {
            return Err(SerpError::Blocked(format!("HTTP status {}", status)));
        }

        let body = resp.text().await?;
        Ok(body)
    }
}
