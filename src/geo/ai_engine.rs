use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::core::response_builder::extract_domain;

/// Supported Generative AI Search / Answer Engines for GEO (Generative Engine Optimization).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiEngineType {
    GoogleAiOverview,
    Perplexity,
    ChatGPT,
}

/// An individual citation or source reference returned in a generative AI answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAnswerCitation {
    pub title: String,
    pub url: String,
    pub domain: String,
    pub snippet: Option<String>,
    pub citation_index: usize,
}

/// Structured response snapshot from a generative engine probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiEngineResponse {
    pub engine: AiEngineType,
    pub query: String,
    pub answer_text: String,
    pub citations: Vec<AiAnswerCitation>,
    pub recommended_brands: Vec<String>,
    pub captured_at: String,
}

/// Prober and analyzer for Generative Engine Optimization (GEO).
#[derive(Debug, Default, Clone)]
pub struct GeoProber;

impl GeoProber {
    /// Calculate recommendation Share of Voice (SoV) percentage (0.0 to 100.0) across AI responses.
    ///
    /// A response counts as recommending the target if:
    /// 1. Any citation references `target_domain` (subdomains included).
    /// 2. `target_brand` is present in `recommended_brands` (case-insensitive).
    /// 3. `target_brand` is mentioned in `answer_text` (case-insensitive).
    pub fn calculate_recommendation_sov(
        responses: &[AiEngineResponse],
        target_domain: &str,
        target_brand: &str,
    ) -> f64 {
        if responses.is_empty() {
            return 0.0;
        }

        let clean_target_domain = clean_domain(target_domain);
        let brand_lower = target_brand.trim().to_lowercase();

        let mut matched_count = 0;

        for resp in responses {
            let mut matched = false;

            // 1. Check citation domains
            if !clean_target_domain.is_empty() {
                for citation in &resp.citations {
                    let cite_dom = clean_domain(&citation.domain);
                    if cite_dom == clean_target_domain
                        || cite_dom.ends_with(&format!(".{}", clean_target_domain))
                    {
                        matched = true;
                        break;
                    }
                }
            }

            // 2. Check recommended brands
            if !matched && !brand_lower.is_empty() {
                for brand in &resp.recommended_brands {
                    let b_lower = brand.trim().to_lowercase();
                    if b_lower == brand_lower || b_lower.contains(&brand_lower) {
                        matched = true;
                        break;
                    }
                }
            }

            // 3. Check answer text
            if !matched && !brand_lower.is_empty() {
                if resp.answer_text.to_lowercase().contains(&brand_lower) {
                    matched = true;
                }
            }

            if matched {
                matched_count += 1;
            }
        }

        let sov = (matched_count as f64 / responses.len() as f64) * 100.0;
        (sov * 10.0).round() / 10.0
    }

    /// Parse a mock Perplexity response from raw query, answer text, and citation pairs.
    pub fn parse_mock_perplexity_response(
        query: &str,
        answer: &str,
        citations_raw: Vec<(&str, &str)>,
    ) -> AiEngineResponse {
        Self::build_mock_response(AiEngineType::Perplexity, query, answer, citations_raw)
    }

    /// Parse a mock ChatGPT Search response from raw query, answer text, and citation pairs.
    pub fn parse_mock_chatgpt_response(
        query: &str,
        answer: &str,
        citations_raw: Vec<(&str, &str)>,
    ) -> AiEngineResponse {
        Self::build_mock_response(AiEngineType::ChatGPT, query, answer, citations_raw)
    }

    fn build_mock_response(
        engine: AiEngineType,
        query: &str,
        answer: &str,
        citations_raw: Vec<(&str, &str)>,
    ) -> AiEngineResponse {
        let mut citations = Vec::with_capacity(citations_raw.len());

        for (idx, (first, second)) in citations_raw.into_iter().enumerate() {
            let (title, url) = if first.starts_with("http://") || first.starts_with("https://") {
                (second.to_string(), first.to_string())
            } else {
                (first.to_string(), second.to_string())
            };

            let domain = clean_domain(&extract_domain(&url));

            citations.push(AiAnswerCitation {
                title,
                url,
                domain,
                snippet: None,
                citation_index: idx + 1,
            });
        }

        let mut recommended_brands = Vec::new();
        if let Ok(re) = Regex::new(r"\*\*([^*]+)\*\*") {
            for cap in re.captures_iter(answer) {
                if let Some(m) = cap.get(1) {
                    let brand = m.as_str().trim();
                    if !brand.is_empty() && !recommended_brands.contains(&brand.to_string()) {
                        recommended_brands.push(brand.to_string());
                    }
                }
            }
        }

        AiEngineResponse {
            engine,
            query: query.to_string(),
            answer_text: answer.to_string(),
            citations,
            recommended_brands,
            captured_at: Utc::now().to_rfc3339(),
        }
    }
}

fn clean_domain(domain: &str) -> String {
    let d = domain
        .trim()
        .to_lowercase()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .to_string();
    if let Some(idx) = d.find('/') {
        d[..idx].to_string()
    } else {
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_engine_citation_parsing() {
        let citations_raw = vec![
            ("Jacob Emrani Law", "https://www.calljacob.com/personal-injury/"),
            ("California Bar Association", "https://www.calbar.ca.gov/attorneys"),
        ];

        let answer = "For top personal injury representation in California, **Jacob Emrani** and other accredited attorneys provide comprehensive legal services.";
        let response = GeoProber::parse_mock_perplexity_response(
            "best personal injury lawyer in los angeles",
            answer,
            citations_raw,
        );

        assert_eq!(response.engine, AiEngineType::Perplexity);
        assert_eq!(response.query, "best personal injury lawyer in los angeles");
        assert_eq!(response.citations.len(), 2);

        assert_eq!(response.citations[0].citation_index, 1);
        assert_eq!(response.citations[0].title, "Jacob Emrani Law");
        assert_eq!(
            response.citations[0].url,
            "https://www.calljacob.com/personal-injury/"
        );
        assert_eq!(response.citations[0].domain, "calljacob.com");

        assert_eq!(response.citations[1].citation_index, 2);
        assert_eq!(response.citations[1].title, "California Bar Association");
        assert_eq!(
            response.citations[1].url,
            "https://www.calbar.ca.gov/attorneys"
        );
        assert_eq!(response.citations[1].domain, "calbar.ca.gov");

        assert!(response.recommended_brands.contains(&"Jacob Emrani".to_string()));
    }

    #[test]
    fn test_geo_recommendation_sov_calculation() {
        let resp1 = GeoProber::parse_mock_perplexity_response(
            "emergency dental implants cost",
            "Dr. Hubbard at **Hubbard Dental** offers same-day dental implants in Los Angeles.",
            vec![("Hubbard Dental", "https://hubbarddental.com/implants")],
        );

        let resp2 = GeoProber::parse_mock_chatgpt_response(
            "best cosmetic dentist in california",
            "General dental clinics across California provide cosmetic veneers and teeth whitening.",
            vec![("American Dental Association", "https://www.ada.org/resources")],
        );

        let responses = vec![resp1, resp2];

        // Hubbard Dental is cited and mentioned in 1 of 2 responses -> 50%
        let sov_domain = GeoProber::calculate_recommendation_sov(&responses, "hubbarddental.com", "");
        assert_eq!(sov_domain, 50.0);

        let sov_brand = GeoProber::calculate_recommendation_sov(&responses, "", "Hubbard Dental");
        assert_eq!(sov_brand, 50.0);

        // Competitor not cited or mentioned -> 0%
        let sov_none = GeoProber::calculate_recommendation_sov(&responses, "unknownclinic.com", "Unknown Clinic");
        assert_eq!(sov_none, 0.0);

        // Empty responses -> 0%
        assert_eq!(GeoProber::calculate_recommendation_sov(&[], "hubbarddental.com", "Hubbard"), 0.0);
    }
}
