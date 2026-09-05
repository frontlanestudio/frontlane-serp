use crate::core::http_client::HttpClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestResponse {
    pub query: String,
    pub engine: String,
    pub suggestions: Vec<String>,
    pub count: usize,
}

#[derive(Clone)]
pub struct SuggestClient {
    http_client: HttpClient,
}

impl SuggestClient {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn suggest(
        &self,
        engine: &str,
        query: &str,
        lang: &str,
        region: &str,
    ) -> Result<SuggestResponse, Box<dyn std::error::Error + Send + Sync>> {
        let norm_engine = engine.to_lowercase();
        let encoded_q = urlencoding_encode(query);
        let hl = if lang.is_empty() { "en" } else { lang };
        let gl = if region.is_empty() { "us" } else { region };

        let url = match norm_engine.as_str() {
            "google" => format!(
                "https://suggestqueries.google.com/complete/search?client=chrome&q={}&hl={}&gl={}",
                encoded_q, hl, gl
            ),
            "bing" => format!(
                "https://api.bing.com/osjson.aspx?query={}&market={}",
                encoded_q, gl
            ),
            "duckduckgo" | "ddg" | "duck" => {
                format!("https://duckduckgo.com/ac/?q={}&type=list", encoded_q)
            }
            "ecosia" => format!("https://ac.ecosia.org/?q={}&type=list", encoded_q),
            _ => format!(
                "https://suggestqueries.google.com/complete/search?client=chrome&q={}&hl={}&gl={}",
                encoded_q, hl, gl
            ),
        };

        let resp = self.http_client.get(&url).await?;
        let text = resp.text().await?;
        let suggestions = parse_opensearch_suggestions(&text);
        let count = suggestions.len();

        Ok(SuggestResponse {
            query: query.to_string(),
            engine: norm_engine,
            suggestions,
            count,
        })
    }
}

fn urlencoding_encode(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
}

pub fn parse_opensearch_suggestions(json_str: &str) -> Vec<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(arr) = v.as_array() {
            if arr.len() >= 2 {
                if let Some(sugg_arr) = arr[1].as_array() {
                    return sugg_arr
                        .iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect();
                }
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opensearch_suggestions() {
        let sample = r#"["rust", ["rust programming", "rust download", "rust language"]]"#;
        let res = parse_opensearch_suggestions(sample);
        assert_eq!(
            res,
            vec!["rust programming", "rust download", "rust language"]
        );
    }
}
