use crate::core::http_client::HttpClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestResponse {
    pub query: String,
    pub engine: String,
    pub suggestions: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone)]
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

pub fn parse_opensearch_xml(xml_str: &str) -> Vec<String> {
    let lower = xml_str.to_lowercase();
    if lower.contains("<error") || lower.contains("<fault") || lower.contains("<html") {
        return Vec::new();
    }

    let mut suggestions = Vec::new();

    // 1. Google Complete XML format: <suggestion data="..."/>
    if let Ok(re_google) = regex::Regex::new(r#"<suggestion\s+[^>]*data\s*=\s*"([^"]+)""#) {
        for cap in re_google.captures_iter(xml_str) {
            if let Some(m) = cap.get(1) {
                suggestions.push(decode_xml_entities(m.as_str()));
            }
        }
    }

    if !suggestions.is_empty() {
        return suggestions;
    }

    // 2. OpenSearch XML format: <Text>...</Text>
    if let Ok(re_opensearch) = regex::Regex::new(r#"<Text[^>]*>([^<]+)</Text>"#) {
        for cap in re_opensearch.captures_iter(xml_str) {
            if let Some(m) = cap.get(1) {
                suggestions.push(decode_xml_entities(m.as_str().trim()));
            }
        }
    }

    suggestions
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
}

pub fn parse_opensearch_suggestions(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
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
        return Vec::new();
    }

    if trimmed.starts_with('<') {
        return parse_opensearch_xml(trimmed);
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opensearch_suggestions_non_empty() {
        // Standard OpenSearch 2-element array
        let sample = r#"["rust", ["rust programming", "rust download", "rust language"]]"#;
        let res = parse_opensearch_suggestions(sample);
        assert_eq!(
            res,
            vec!["rust programming", "rust download", "rust language"]
        );

        // Rich OpenSearch 4-element array (Google Chrome format)
        let sample_chrome = r#"["rust", ["rust book", "rustup", "rust docs"], ["", "", ""], [], {"google:clientdata":{"bpc":false}}]"#;
        let res_chrome = parse_opensearch_suggestions(sample_chrome);
        assert_eq!(res_chrome, vec!["rust book", "rustup", "rust docs"]);

        // Mixed types in suggestions array: skips non-string elements gracefully
        let sample_mixed = r#"["query", ["valid string", 123, null, true, "another string"]]"#;
        let res_mixed = parse_opensearch_suggestions(sample_mixed);
        assert_eq!(res_mixed, vec!["valid string", "another string"]);
    }

    #[test]
    fn test_parse_opensearch_suggestions_empty() {
        // Empty JSON array
        assert!(parse_opensearch_suggestions("[]").is_empty());

        // Single element array (missing suggestions sub-array)
        assert!(parse_opensearch_suggestions(r#"["only query"]"#).is_empty());

        // Empty suggestions sub-array
        assert!(parse_opensearch_suggestions(r#"["query", []]"#).is_empty());

        // Empty string and whitespace
        assert!(parse_opensearch_suggestions("").is_empty());
        assert!(parse_opensearch_suggestions("   \n\t  ").is_empty());

        // Invalid JSON structures
        assert!(parse_opensearch_suggestions("{}").is_empty());
        assert!(parse_opensearch_suggestions(r#"{"suggestions": ["a", "b"]}"#).is_empty());
        assert!(parse_opensearch_suggestions("not json at all").is_empty());
    }

    #[test]
    fn test_parse_opensearch_xml_formats() {
        // Google Complete XML format
        let google_xml = r#"<?xml version="1.0"?>
<toplevel>
  <CompleteSuggestion>
    <suggestion data="rust programming"/>
  </CompleteSuggestion>
  <CompleteSuggestion>
    <suggestion data="rust &amp; web"/>
  </CompleteSuggestion>
  <CompleteSuggestion>
    <suggestion data="rust &quot;async&quot;"/>
  </CompleteSuggestion>
</toplevel>"#;
        let res_google = parse_opensearch_suggestions(google_xml);
        assert_eq!(
            res_google,
            vec![
                "rust programming",
                "rust & web",
                "rust \"async\""
            ]
        );

        // OpenSearch 2.0 XML format
        let opensearch_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<SearchSuggestion version="2.0" xmlns="http://opensearch.org/searchsuggest2">
  <Query>rust</Query>
  <Section>
    <Item>
      <Text>rust tutorial</Text>
    </Item>
    <Item>
      <Text>rust lang</Text>
    </Item>
  </Section>
</SearchSuggestion>"#;
        let res_os = parse_opensearch_suggestions(opensearch_xml);
        assert_eq!(res_os, vec!["rust tutorial", "rust lang"]);
    }

    #[test]
    fn test_parse_xml_error_handling() {
        // XML API error response
        let xml_error = r#"<?xml version="1.0" encoding="UTF-8"?>
<error>
  <code>429</code>
  <message>Rate limit exceeded</message>
</error>"#;
        assert!(parse_opensearch_suggestions(xml_error).is_empty());

        // SOAP fault
        let soap_fault = r#"<soap:Envelope>
  <soap:Body>
    <soap:Fault>
      <faultcode>soap:Server</faultcode>
      <faultstring>Internal error</faultstring>
    </soap:Fault>
  </soap:Body>
</soap:Envelope>"#;
        assert!(parse_opensearch_suggestions(soap_fault).is_empty());

        // HTML error page
        let html_error = r#"<!DOCTYPE html>
<html>
<head><title>503 Service Unavailable</title></head>
<body><h1>503 Service Unavailable</h1></body>
</html>"#;
        assert!(parse_opensearch_suggestions(html_error).is_empty());

        // Malformed XML should not panic
        let malformed_xml = r#"<toplevel><suggestion data="broken"#;
        assert!(parse_opensearch_suggestions(malformed_xml).is_empty());
    }

    #[test]
    fn test_urlencoding_formatting() {
        assert_eq!(urlencoding_encode("rust"), "rust");
        assert_eq!(urlencoding_encode("rust programming"), "rust+programming");
        assert_eq!(urlencoding_encode("c++ & python"), "c%2B%2B+%26+python");
        assert_eq!(urlencoding_encode("café"), "caf%C3%A9");
        assert_eq!(urlencoding_encode("a/b?c=d#e"), "a%2Fb%3Fc%3Dd%23e");
    }

    #[test]
    fn test_suggest_response_formatting_and_serde() {
        let resp = SuggestResponse {
            query: "rust".to_string(),
            engine: "google".to_string(),
            suggestions: vec!["rust lang".to_string(), "rustup".to_string()],
            count: 2,
        };

        // Check formatting debug output
        let debug_str = format!("{:?}", resp);
        assert!(debug_str.contains("query: \"rust\""));
        assert!(debug_str.contains("engine: \"google\""));

        // Serialization
        let json = serde_json::to_string(&resp).expect("Serialization should succeed");
        assert!(json.contains("\"query\":\"rust\""));
        assert!(json.contains("\"engine\":\"google\""));
        assert!(json.contains("\"count\":2"));
        assert!(json.contains("\"suggestions\":[\"rust lang\",\"rustup\"]"));

        // Deserialization
        let parsed: SuggestResponse =
            serde_json::from_str(&json).expect("Deserialization should succeed");
        assert_eq!(parsed.query, "rust");
        assert_eq!(parsed.engine, "google");
        assert_eq!(parsed.suggestions, vec!["rust lang", "rustup"]);
        assert_eq!(parsed.count, 2);
    }
}
