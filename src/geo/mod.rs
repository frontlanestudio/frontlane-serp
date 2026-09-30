pub mod ai_engine;

pub use ai_engine::{AiAnswerCitation, AiEngineResponse, AiEngineType, GeoProber};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::response_builder::extract_domain;
use crate::core::types::{ResultType, SearchResult, SerpFeature};

/// Share of Voice statistics for a domain cited in generative AI answers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainCitationShare {
    pub domain: String,
    pub citations_count: usize,
    pub share_of_voice_pct: f64,
    pub sample_urls: Vec<String>,
}

/// An individual source citation embedded in an AI Overview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoCitationItem {
    pub position: usize,
    pub domain: String,
    pub url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
}

/// Comprehensive Generative Engine Optimization (GEO) & AI Citation Share of Voice Report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoCitationsReport {
    pub query: String,
    pub engine: String,
    pub has_ai_overview: bool,
    pub summary: Option<String>,
    pub total_citations: usize,
    pub unique_domains_count: usize,
    pub domains: Vec<DomainCitationShare>,
    pub citations: Vec<GeoCitationItem>,
    pub target_domain: Option<String>,
    pub target_cited: bool,
    pub target_share_of_voice_pct: f64,
    pub target_citations_count: usize,
    pub recommendations: Vec<String>,
    pub analyzed_at: String,
}

impl GeoCitationsReport {
    /// Render summary report to standard output.
    pub fn print_summary(&self, format: &str) {
        if format.eq_ignore_ascii_case("json") {
            println!("{}", serde_json::to_string_pretty(self).unwrap_or_default());
            return;
        }

        println!("================================================================================");
        println!(" GENERATIVE ENGINE OPTIMIZATION (GEO) — AI CITATION SHARE OF VOICE");
        println!("================================================================================");
        println!(" Query:           {}", self.query);
        println!(" Engine:          {}", self.engine.to_uppercase());
        println!(
            " AI Overview:     {}",
            if self.has_ai_overview {
                "DETECTED"
            } else {
                "NOT DETECTED"
            }
        );
        println!(
            " Citations:       {} total sources cited across {} unique domains",
            self.total_citations, self.unique_domains_count
        );

        if let Some(ref sum) = self.summary {
            println!("--------------------------------------------------------------------------------");
            println!(" AI OVERVIEW SYNTHESIS:");
            let preview = if sum.len() > 300 {
                format!("{}...", &sum[..300])
            } else {
                sum.clone()
            };
            println!(" \"{}\"", preview.trim());
        }

        println!("--------------------------------------------------------------------------------");
        println!(" DOMAIN SHARE OF VOICE (SoV) RANKINGS:");
        if self.domains.is_empty() {
            println!("   (No cited sources found in the AI overview)");
        } else {
            for (idx, dom) in self.domains.iter().enumerate() {
                println!(
                    "   #{:<2} {:<30} {:>5.1}% ({} citations)",
                    idx + 1,
                    dom.domain,
                    dom.share_of_voice_pct,
                    dom.citations_count
                );
            }
        }

        if let Some(ref target) = self.target_domain {
            println!("--------------------------------------------------------------------------------");
            println!(" TARGET DOMAIN ANALYSIS: {}", target);
            println!(
                " Cited in AI Overview: {}",
                if self.target_cited {
                    "YES (PASSED)"
                } else {
                    "NO (GAP DETECTED)"
                }
            );
            println!(
                " Share of Voice:       {:.1}% ({} citations)",
                self.target_share_of_voice_pct, self.target_citations_count
            );
        }

        println!("--------------------------------------------------------------------------------");
        println!(" GEO ACTIONABLE RECOMMENDATIONS:");
        for rec in &self.recommendations {
            println!(" • {}", rec);
        }
        println!("================================================================================");
    }
}

/// Analyze features directly to produce a GEO citation report.
pub fn analyze_features(
    query: &str,
    engine: &str,
    features: &[SerpFeature],
    target: Option<&str>,
) -> GeoCitationsReport {
    let ai_feature = features
        .iter()
        .find(|f| f.feature_type == ResultType::AiSummary || f.title.as_deref() == Some("AI Overview"));

    let has_ai_overview = ai_feature.is_some();
    let summary = ai_feature.and_then(|f| f.text.clone());

    let mut citations = Vec::new();
    let mut domain_counts: HashMap<String, (usize, Vec<String>)> = HashMap::new();

    if let Some(ai) = ai_feature {
        // Collect from items first
        for item in &ai.items {
            let url = item.link.clone().unwrap_or_default();
            let domain = if let Some(ref d) = item.domain {
                clean_domain(d)
            } else if !url.is_empty() {
                clean_domain(&extract_domain(&url))
            } else {
                continue;
            };

            if domain.is_empty() {
                continue;
            }

            let entry = domain_counts.entry(domain.clone()).or_insert((0, Vec::new()));
            entry.0 += 1;
            if !url.is_empty() && !entry.1.contains(&url) {
                entry.1.push(url.clone());
            }

            citations.push(GeoCitationItem {
                position: citations.len() + 1,
                domain,
                url,
                title: item.title.clone(),
                snippet: item.snippet.clone().or_else(|| item.text.clone()),
            });
        }

        // Also inspect links if items had fewer or none
        if citations.is_empty() {
            for link in &ai.links {
                let url = link.url.clone().unwrap_or_default();
                let domain = clean_domain(&extract_domain(&url));
                if domain.is_empty() {
                    continue;
                }

                let entry = domain_counts.entry(domain.clone()).or_insert((0, Vec::new()));
                entry.0 += 1;
                if !url.is_empty() && !entry.1.contains(&url) {
                    entry.1.push(url.clone());
                }

                citations.push(GeoCitationItem {
                    position: citations.len() + 1,
                    domain,
                    url,
                    title: link.title.clone(),
                    snippet: None,
                });
            }
        }
    }

    let total_citations = citations.len();
    let mut domain_shares = Vec::new();

    for (domain, (count, sample_urls)) in domain_counts {
        let share_of_voice_pct = if total_citations > 0 {
            ((count as f64 / total_citations as f64) * 1000.0).round() / 10.0
        } else {
            0.0
        };
        domain_shares.push(DomainCitationShare {
            domain,
            citations_count: count,
            share_of_voice_pct,
            sample_urls,
        });
    }

    // Sort descending by citation count, then domain name
    domain_shares.sort_by(|a, b| {
        b.citations_count
            .cmp(&a.citations_count)
            .then_with(|| a.domain.cmp(&b.domain))
    });

    let unique_domains_count = domain_shares.len();

    // Check target domain
    let target_domain = target.map(|t| clean_domain(t));
    let mut target_cited = false;
    let mut target_share_of_voice_pct = 0.0;
    let mut target_citations_count = 0;

    if let Some(ref tgt) = target_domain {
        if let Some(matched) = domain_shares.iter().find(|d| d.domain == *tgt || d.domain.ends_with(&format!(".{}", tgt))) {
            target_cited = true;
            target_share_of_voice_pct = matched.share_of_voice_pct;
            target_citations_count = matched.citations_count;
        }
    }

    let recommendations = generate_recommendations(
        has_ai_overview,
        total_citations,
        target_domain.as_deref(),
        target_cited,
        target_share_of_voice_pct,
        &domain_shares,
    );

    GeoCitationsReport {
        query: query.to_string(),
        engine: engine.to_string(),
        has_ai_overview,
        summary,
        total_citations,
        unique_domains_count,
        domains: domain_shares,
        citations,
        target_domain,
        target_cited,
        target_share_of_voice_pct,
        target_citations_count,
        recommendations,
        analyzed_at: Utc::now().to_rfc3339(),
    }
}

/// Analyze full SERP search results to produce a GEO report.
pub fn analyze_results(
    query: &str,
    engine: &str,
    results: &[SearchResult],
    target: Option<&str>,
) -> GeoCitationsReport {
    // Collect features from results
    let mut all_features = Vec::new();
    for res in results {
        all_features.extend(res.features.clone());
    }
    analyze_features(query, engine, &all_features, target)
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

fn generate_recommendations(
    has_ai_overview: bool,
    total_citations: usize,
    target_domain: Option<&str>,
    target_cited: bool,
    target_sov: f64,
    domains: &[DomainCitationShare],
) -> Vec<String> {
    let mut recs = Vec::new();

    if !has_ai_overview {
        recs.push("No AI Overview triggered for this query. Generative engines tend to trigger for informational, comparative, and troubleshooting queries ('what is', 'cost of', 'vs', 'reviews').".to_string());
        recs.push("Target conversational and long-tail query variations to increase chances of capturing emerging AI summary real estate.".to_string());
        return recs;
    }

    if total_citations == 0 {
        recs.push("AI Overview is present but did not display granular source citation links in this SERP snapshot.".to_string());
        return recs;
    }

    let top_domains = domains
        .iter()
        .take(3)
        .map(|d| format!("{} ({:.1}%)", d.domain, d.share_of_voice_pct))
        .collect::<Vec<_>>()
        .join(", ");

    recs.push(format!(
        "High authoritative citation concentration observed among leaders: {}",
        top_domains
    ));

    if let Some(target) = target_domain {
        if !target_cited {
            recs.push(format!(
                "Citation Gap: '{}' has 0% AI Citation Share of Voice. Generative engines prioritize structured tables, bullet lists, and direct 40-60 word definitive answers.",
                target
            ));
            recs.push("Implement FAQPage and Article schema markup to allow LLM crawlers to ingest entity answers directly.".to_string());
            recs.push("Publish proprietary statistics, research benchmarks, or case study figures that generative models seek as primary citation sources.".to_string());
        } else if target_sov < 30.0 {
            recs.push(format!(
                "Moderate Presence: '{}' holds {:.1}% Share of Voice. Broaden topical coverage with supporting procedural sub-pages to win multi-source synthesis slots.",
                target, target_sov
            ));
        } else {
            recs.push(format!(
                "Dominant Authority: '{}' commands {:.1}% Share of Voice in AI Overviews. Defend this position by maintaining freshness dates and author credentials (E-E-A-T).",
                target, target_sov
            ));
        }
    } else {
        recs.push("Structure key informational landing pages with clear, unambiguous H2/H3 question headers followed by concise 2-sentence executive answers.".to_string());
        recs.push("Include citations to academic or primary source data to increase generative answer retrieval confidence.".to_string());
    }

    recs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{FeatureItem, ResultType};

    #[test]
    fn test_geo_analysis_with_ai_citations() {
        let features = vec![SerpFeature {
            id: "geo-test-1".to_string(),
            engine: "google".to_string(),
            feature_type: ResultType::AiSummary,
            title: Some("AI Overview".to_string()),
            text: Some("Dental implants are titanium posts surgically positioned into the jawbone beneath your gums.".to_string()),
            items: vec![
                FeatureItem {
                    title: Some("Dental Implants Overview".to_string()),
                    link: Some("https://www.mayoclinic.org/tests-procedures/dental-implant-surgery/about/pac-20384622".to_string()),
                    domain: Some("www.mayoclinic.org".to_string()),
                    text: None,
                    snippet: Some("Implants fuse with bone tissue.".to_string()),
                    ..Default::default()
                },
                FeatureItem {
                    title: Some("What to Know About Dental Implants".to_string()),
                    link: Some("https://www.colgate.com/en-us/oral-health/implants/what-are-dental-implants".to_string()),
                    domain: Some("colgate.com".to_string()),
                    text: None,
                    snippet: Some("Overview of procedures and care.".to_string()),
                    ..Default::default()
                },
                FeatureItem {
                    title: Some("Dental Implants Procedures & Cost".to_string()),
                    link: Some("https://www.mayoclinic.org/tests-procedures/dental-implant-surgery/cost".to_string()),
                    domain: Some("mayoclinic.org".to_string()),
                    text: None,
                    snippet: Some("Breakdown of surgery costs.".to_string()),
                    ..Default::default()
                },
            ],
            links: vec![],
            source_result_ids: vec![],
            position: None,
            confidence: None,
            extracted_at: Utc::now().to_rfc3339(),
        }];

        let report = analyze_features(
            "dental implants cost and procedure",
            "google",
            &features,
            Some("colgate.com"),
        );

        assert!(report.has_ai_overview);
        assert_eq!(report.total_citations, 3);
        assert_eq!(report.unique_domains_count, 2);
        assert_eq!(report.domains[0].domain, "mayoclinic.org");
        assert_eq!(report.domains[0].citations_count, 2);
        assert_eq!(report.domains[0].share_of_voice_pct, 66.7);

        assert!(report.target_cited);
        assert_eq!(report.target_citations_count, 1);
        assert_eq!(report.target_share_of_voice_pct, 33.3);
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn test_geo_analysis_target_not_cited() {
        let features = vec![SerpFeature {
            id: "geo-test-2".to_string(),
            engine: "google".to_string(),
            feature_type: ResultType::AiSummary,
            title: Some("AI Overview".to_string()),
            text: Some("Here is an overview of commercial litigation.".to_string()),
            items: vec![
                FeatureItem {
                    title: Some("Commercial Litigation Guide".to_string()),
                    link: Some("https://www.findlaw.com/litigation".to_string()),
                    domain: Some("findlaw.com".to_string()),
                    ..Default::default()
                }
            ],
            links: vec![],
            source_result_ids: vec![],
            position: None,
            confidence: None,
            extracted_at: Utc::now().to_rfc3339(),
        }];

        let report = analyze_features(
            "commercial litigation process",
            "google",
            &features,
            Some("calljacob.com"),
        );

        assert!(report.has_ai_overview);
        assert!(!report.target_cited);
        assert_eq!(report.target_share_of_voice_pct, 0.0);
        assert!(report.recommendations.iter().any(|r| r.contains("Citation Gap")));
    }
}
