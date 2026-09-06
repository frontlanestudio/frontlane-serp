use crate::audit::types::{GapInsight, PageAuditResult, SerpBenchmark};
use std::collections::HashMap;

pub fn analyze_gaps(
    keyword: &str,
    target_audit: Option<&PageAuditResult>,
    competitors: &[PageAuditResult],
) -> (SerpBenchmark, Vec<GapInsight>) {
    let comp_count = competitors.len();
    if comp_count == 0 {
        return (SerpBenchmark::default(), Vec::new());
    }

    let mut total_words = 0;
    let mut max_words = 0;
    let mut min_words = usize::MAX;
    let mut title_exact_hits = 0;
    let mut h1_exact_hits = 0;
    let mut dedicated_slug_hits = 0;
    let mut schema_hits = 0;
    let mut schema_freq: HashMap<String, usize> = HashMap::new();

    for comp in competitors {
        total_words += comp.word_count;
        max_words = max_words.max(comp.word_count);
        min_words = min_words.min(comp.word_count);
        if comp.title_exact_match {
            title_exact_hits += 1;
        }
        if comp.h1_exact_match {
            h1_exact_hits += 1;
        }
        if comp.is_dedicated_page || comp.slug_has_exact_kw || comp.slug_has_geo {
            dedicated_slug_hits += 1;
        }
        if !comp.schema_types.is_empty() {
            schema_hits += 1;
        }
        for st in &comp.schema_types {
            *schema_freq.entry(st.clone()).or_insert(0) += 1;
        }
    }

    let avg_word_count = total_words / comp_count;
    let min_word_count = if min_words == usize::MAX { 0 } else { min_words };
    let exact_title_match_pct = (title_exact_hits as f64 / comp_count as f64) * 100.0;
    let exact_h1_match_pct = (h1_exact_hits as f64 / comp_count as f64) * 100.0;
    let dedicated_slug_pct = (dedicated_slug_hits as f64 / comp_count as f64) * 100.0;
    let schema_adoption_pct = (schema_hits as f64 / comp_count as f64) * 100.0;

    let mut schema_sorted: Vec<(String, usize)> = schema_freq.into_iter().collect();
    schema_sorted.sort_by_key(|a| std::cmp::Reverse(a.1));
    let common_schema_types = schema_sorted.into_iter().take(5).map(|(s, _)| s).collect();

    let benchmarks = SerpBenchmark {
        total_competitors_analyzed: comp_count,
        avg_word_count,
        max_word_count: max_words,
        min_word_count,
        exact_title_match_pct,
        exact_h1_match_pct,
        dedicated_slug_pct,
        schema_adoption_pct,
        common_schema_types,
    };

    let mut insights = Vec::new();

    if let Some(target) = target_audit {
        // 1. Content Depth Gap
        if target.word_count < (avg_word_count as f64 * 0.75) as usize {
            let diff = avg_word_count.saturating_sub(target.word_count);
            insights.push(GapInsight {
                category: "Content Depth".to_string(),
                severity: "High".to_string(),
                observation: format!(
                    "Target page has {} words vs top competitor average of {} words (deficit of ~{} words; max competitor has {} words).",
                    target.word_count, avg_word_count, diff, max_words
                ),
                actionable_recommendation: format!(
                    "Expand page content to at least {} words by adding localized jurisdiction guidelines, localized traffic accident stats, settlement case studies, and comprehensive FAQ accordions.",
                    avg_word_count
                ),
            });
        }

        // 2. Title Tag Alignment
        if !target.title_exact_match {
            insights.push(GapInsight {
                category: "Title Tag".to_string(),
                severity: "High".to_string(),
                observation: format!(
                    "Target title ('{}') does not contain the exact query phrase '{}'. {:.0}% of top competitors include the exact keyword.",
                    target.title, keyword, exact_title_match_pct
                ),
                actionable_recommendation: format!(
                    "Front-load the exact keyword in your <title> tag: '<title>{} | [Brand / Value Proposition]</title>'.",
                    keyword
                ),
            });
        } else if !target.title_starts_with_kw {
            insights.push(GapInsight {
                category: "Title Tag Prominence".to_string(),
                severity: "Medium".to_string(),
                observation: "Target title contains the keyword, but does not start with it. Search engines give higher ranking weight to keywords positioned in the first 30 characters of the title.".to_string(),
                actionable_recommendation: format!(
                    "Place '{}' at the very beginning of the <title> tag.",
                    keyword
                ),
            });
        }

        // 3. H1 Heading Alignment
        if !target.h1_exact_match {
            let current_h1 = target.h1.first().cloned().unwrap_or_else(|| "None".to_string());
            insights.push(GapInsight {
                category: "H1 Heading".to_string(),
                severity: "High".to_string(),
                observation: format!(
                    "Target primary <h1> is '{}', which lacks the exact target keyword '{}'. {:.0}% of top ranking competitors use the exact keyword in their <h1>.",
                    current_h1, keyword, exact_h1_match_pct
                ),
                actionable_recommendation: format!(
                    "Change the primary <h1> to directly match the target search intent: '<h1>{}</h1>'.",
                    keyword
                ),
            });
        }

        // 4. URL Architecture & Geo-Intent
        if !target.is_dedicated_page && dedicated_slug_pct >= 50.0 {
            insights.push(GapInsight {
                category: "URL Architecture".to_string(),
                severity: "High".to_string(),
                observation: format!(
                    "{:.0}% of top ranking competitors rank with dedicated localized landing pages rather than a generic root homepage.",
                    dedicated_slug_pct
                ),
                actionable_recommendation: "Target this query with a dedicated localized landing page (e.g. '/los-angeles-personal-injury-lawyer' or '/areas-we-serve/los-angeles-personal-injury-lawyer/') rather than the root domain homepage, and point prominent internal links to it.".to_string(),
            });
        }

        // 5. Structured Data Schema
        if !target.has_local_business_schema && schema_adoption_pct >= 40.0 {
            insights.push(GapInsight {
                category: "Structured Data".to_string(),
                severity: "Medium".to_string(),
                observation: format!(
                    "Target is missing LocalBusiness / LegalService / Attorney JSON-LD schema markup. {:.0}% of competitors implement rich schema.",
                    schema_adoption_pct
                ),
                actionable_recommendation: "Implement JSON-LD LegalService/Attorney schema specifying addressLocality, geo coordinates (latitude/longitude), openingHours, and areaServed.".to_string(),
            });
        }
    }

    (benchmarks, insights)
}
