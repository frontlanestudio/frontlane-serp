use url::Url;
use crate::core::http_client::HttpClient;

const LLMS_TXT_CANDIDATES: &[&str] = &["/llms-full.txt", "/llms.txt"];
const MIN_LLMS_TXT_CHARS: usize = 200;

pub fn is_site_root(raw_url: &str) -> bool {
    if let Ok(u) = Url::parse(raw_url) {
        let path = u.path().trim_matches('/');
        return path.is_empty();
    }
    false
}

pub async fn try_llms_txt(client: &HttpClient, raw_url: &str) -> Option<(String, String)> {
    if !is_site_root(raw_url) {
        return None;
    }
    let base = Url::parse(raw_url).ok()?;

    for candidate in LLMS_TXT_CANDIDATES {
        if let Ok(target_url) = base.join(candidate) {
            if let Ok(content) = client.fetch(target_url.as_str(), None).await {
                let trimmed = content.trim();
                if trimmed.len() >= MIN_LLMS_TXT_CHARS
                    && !trimmed.starts_with("<!DOCTYPE")
                    && !trimmed.starts_with("<html")
                    && !trimmed.starts_with("<body")
                {
                    return Some((target_url.to_string(), trimmed.to_string()));
                }
            }
        }
    }
    None
}
