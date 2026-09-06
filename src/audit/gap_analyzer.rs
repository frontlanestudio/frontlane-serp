use crate::audit::types::{GapInsight, PageAuditResult, SerpBenchmark};
use std::collections::HashMap;

pub struct GapAnalysisOutput {
    pub benchmarks: SerpBenchmark,
    pub insights: Vec<GapInsight>,
    pub opportunity_score: u32,
    pub quick_wins: Vec<String>,
    pub missing_content_outline: Vec<String>,
}

pub fn analyze_gaps(
    keyword: &str,
    target_audit: Option<&PageAuditResult>,
    competitors: &[PageAuditResult],
    has_local_pack: bool,
    has_paa: bool,
) -> GapAnalysisOutput {
    let comp_count = competitors.len();
    if comp_count == 0 {
        return GapAnalysisOutput {
            benchmarks: SerpBenchmark::default(),
            insights: Vec::new(),
            opportunity_score: 50,
            quick_wins: Vec::new(),
            missing_content_outline: Vec::new(),
        };
    }

    let mut total_words = 0;
    let mut max_words = 0;
    let mut min_words = usize::MAX;
    let mut title_exact_hits = 0;
    let mut h1_exact_hits = 0;
    let mut dedicated_slug_hits = 0;
    let mut directory_hits = 0;
    let mut schema_hits = 0;
    let mut faq_hits = 0;
    let mut total_tel = 0;
    let mut total_forms = 0;

    let mut schema_freq: HashMap<String, usize> = HashMap::new();
    let mut topic_freq: HashMap<String, usize> = HashMap::new();
    let mut competitor_questions: Vec<String> = Vec::new();

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
        if comp.is_directory_aggregator {
            directory_hits += 1;
        }
        if !comp.schema_types.is_empty() {
            schema_hits += 1;
        }
        if comp.has_faq_schema {
            faq_hits += 1;
        }
        total_tel += comp.tel_links_count;
        total_forms += comp.form_count;

        for st in &comp.schema_types {
            *schema_freq.entry(st.clone()).or_insert(0) += 1;
        }

        for h2 in &comp.h2_headings {
            let clean = clean_heading(h2);
            if clean.len() >= 5 {
                *topic_freq.entry(clean).or_insert(0) += 1;
            }
        }

        for q in &comp.questions_found {
            if !competitor_questions.contains(q) && competitor_questions.len() < 10 {
                competitor_questions.push(q.clone());
            }
        }
    }

    let avg_word_count = total_words / comp_count;
    let min_word_count = if min_words == usize::MAX { 0 } else { min_words };
    let exact_title_match_pct = (title_exact_hits as f64 / comp_count as f64) * 100.0;
    let exact_h1_match_pct = (h1_exact_hits as f64 / comp_count as f64) * 100.0;
    let dedicated_slug_pct = (dedicated_slug_hits as f64 / comp_count as f64) * 100.0;
    let directory_aggregator_pct = (directory_hits as f64 / comp_count as f64) * 100.0;
    let schema_adoption_pct = (schema_hits as f64 / comp_count as f64) * 100.0;
    let faq_schema_adoption_pct = (faq_hits as f64 / comp_count as f64) * 100.0;
    let avg_tel_links = total_tel as f64 / comp_count as f64;
    let avg_form_count = total_forms as f64 / comp_count as f64;

    let mut schema_sorted: Vec<(String, usize)> = schema_freq.into_iter().collect();
    schema_sorted.sort_by_key(|a| std::cmp::Reverse(a.1));
    let common_schema_types = schema_sorted.into_iter().take(5).map(|(s, _)| s).collect();

    let mut topic_sorted: Vec<(String, usize)> = topic_freq.into_iter().collect();
    topic_sorted.sort_by_key(|a| std::cmp::Reverse(a.1));
    let top_competitor_topics: Vec<String> = topic_sorted
        .into_iter()
        .filter(|(_, count)| *count >= 2 || comp_count <= 3)
        .take(8)
        .map(|(t, _)| t)
        .collect();

    let benchmarks = SerpBenchmark {
        total_competitors_analyzed: comp_count,
        avg_word_count,
        max_word_count: max_words,
        min_word_count,
        exact_title_match_pct,
        exact_h1_match_pct,
        dedicated_slug_pct,
        directory_aggregator_pct,
        schema_adoption_pct,
        faq_schema_adoption_pct,
        avg_tel_links,
        avg_form_count,
        common_schema_types,
        top_competitor_topics: top_competitor_topics.clone(),
        competitor_questions: competitor_questions.clone(),
        has_local_pack_in_serp: has_local_pack,
        has_paa_in_serp: has_paa,
    };

    let mut insights = Vec::new();
    let mut quick_wins = Vec::new();
    let mut missing_content_outline = Vec::new();
    let mut score: i32 = 100;

    if let Some(target) = target_audit {
        // Missing Topics calculation
        let target_headings_lower: Vec<String> = target
            .h2_headings
            .iter()
            .chain(target.h3_headings.iter())
            .map(|h| h.to_lowercase())
            .collect();

        for topic in &top_competitor_topics {
            let tl = topic.to_lowercase();
            if !target_headings_lower.iter().any(|th| th.contains(&tl) || tl.contains(th)) {
                missing_content_outline.push(topic.clone());
            }
        }

        // 1. Directory Aggregator SERP Intent Check
        if directory_aggregator_pct >= 40.0 {
            insights.push(GapInsight {
                category: "SERP Search Intent".to_string(),
                severity: "High".to_string(),
                observation: format!(
                    "{:.0}% of top organic positions are occupied by lawyer directories & aggregators (Justia, Avvo, FindLaw, Yelp).",
                    directory_aggregator_pct
                ),
                actionable_recommendation: "Query intent is heavily directory-driven. Maintain a dual strategy: optimize Google Business Profile, Justia, and Avvo listings to capture directory traffic while building deep localized content for organic capture.".to_string(),
            });
        }

        // 2. Title Tag Gap
        if !target.title_exact_match {
            score -= 20;
            quick_wins.push(format!("Front-load exact keyword in <title>: '<title>{} | [Firm Name]</title>'", keyword));
            insights.push(GapInsight {
                category: "Title Tag Alignment".to_string(),
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
            score -= 5;
            quick_wins.push(format!("Move '{}' to the beginning of the <title> tag.", keyword));
            insights.push(GapInsight {
                category: "Title Tag Prominence".to_string(),
                severity: "Medium".to_string(),
                observation: "Target title contains the keyword, but does not start with it. Search engines give higher ranking weight to keywords positioned in the first 30 characters of the title.".to_string(),
                actionable_recommendation: format!("Place '{}' at the very beginning of the <title> tag.", keyword),
            });
        }

        // 3. H1 Heading Alignment
        if !target.h1_exact_match {
            score -= 15;
            quick_wins.push(format!("Update <h1> to match search intent: '<h1>{}</h1>'", keyword));
            let current_h1 = target.h1.first().cloned().unwrap_or_else(|| "None".to_string());
            insights.push(GapInsight {
                category: "H1 Heading Alignment".to_string(),
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

        // 4. Content Depth Gap
        if target.word_count < (avg_word_count as f64 * 0.75) as usize {
            let diff = avg_word_count.saturating_sub(target.word_count);
            score -= 20;
            insights.push(GapInsight {
                category: "Content Depth".to_string(),
                severity: "High".to_string(),
                observation: format!(
                    "Target page has {} words vs top competitor average of {} words (deficit of ~{} words; max competitor has {} words).",
                    target.word_count, avg_word_count, diff, max_words
                ),
                actionable_recommendation: format!(
                    "Expand page content to at least {} words by adding comprehensive sections addressing key competitor subtopics.",
                    avg_word_count
                ),
            });
        }

        // 5. Topical & Outline Subtopics Gap
        if !missing_content_outline.is_empty() {
            score -= 10;
            quick_wins.push(format!("Add missing subtopic sections in H2: {}", missing_content_outline.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
            insights.push(GapInsight {
                category: "Topical Coverage".to_string(),
                severity: "Medium".to_string(),
                observation: format!(
                    "Competitors frequently cover {} subtopics that are missing from the target page (e.g. '{}').",
                    missing_content_outline.len(),
                    missing_content_outline.iter().take(4).cloned().collect::<Vec<_>>().join("', '")
                ),
                actionable_recommendation: format!(
                    "Add dedicated H2 sections covering these missing competitor topics: {}.",
                    missing_content_outline.join(", ")
                ),
            });
        }

        // 6. URL Architecture & Geo-Intent
        if !target.is_dedicated_page && dedicated_slug_pct >= 50.0 {
            score -= 15;
            quick_wins.push("Deploy a dedicated localized landing page (e.g. /los-angeles-personal-injury-lawyer)".to_string());
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

        // 7. FAQ & Question Answering
        if !target.has_faq_schema && (!competitor_questions.is_empty() || faq_schema_adoption_pct >= 30.0) {
            score -= 5;
            quick_wins.push("Add an FAQ accordion with FAQPage JSON-LD schema.".to_string());
            insights.push(GapInsight {
                category: "FAQ & Question Answering".to_string(),
                severity: "Medium".to_string(),
                observation: format!(
                    "Target page lacks structured FAQ schema. Competitors answer key user questions (e.g. '{}').",
                    competitor_questions.first().cloned().unwrap_or_else(|| "Common questions".to_string())
                ),
                actionable_recommendation: "Add an FAQ section answering top searcher questions with FAQPage JSON-LD markup to capture rich SERP accordion real estate.".to_string(),
            });
        }

        // 8. Structured Local Data & Reviews
        if !target.has_local_business_schema && schema_adoption_pct >= 40.0 {
            score -= 5;
            quick_wins.push("Add JSON-LD LegalService / Attorney schema with address and geo-coordinates.".to_string());
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

        // 9. Review & Rating Trust Gap
        if target.rating_value.is_none() && target.review_count.is_none() {
            let comp_with_reviews = competitors.iter().filter_map(|c| c.review_count).max();
            if let Some(max_rev) = comp_with_reviews {
                insights.push(GapInsight {
                    category: "Review & Trust Proof".to_string(),
                    severity: "Low".to_string(),
                    observation: format!(
                        "Target does not display AggregateRating schema. Top competitor highlights {} verified reviews in schema.",
                        max_rev
                    ),
                    actionable_recommendation: "Embed client review ratings and output AggregateRating JSON-LD schema to earn star ratings in search snippets and boost CTR.".to_string(),
                });
            }
        }

        // 10. Indexability & Canonical Check
        if target.is_noindex {
            score = 0;
            quick_wins.insert(0, "CRITICAL: Remove 'noindex' robots meta tag immediately!".to_string());
            insights.insert(0, GapInsight {
                category: "Indexability".to_string(),
                severity: "Critical".to_string(),
                observation: "Target page has a 'noindex' robots meta tag directive.".to_string(),
                actionable_recommendation: "Remove the 'noindex' directive from your page header so search engines can crawl and rank it.".to_string(),
            });
        }

        // 11. Conversion Call-to-Action Signals
        if target.tel_links_count == 0 {
            insights.push(GapInsight {
                category: "Conversion Architecture".to_string(),
                severity: "Low".to_string(),
                observation: format!(
                    "Target page has 0 clickable 'tel:' links vs competitor average of {:.1} phone links.",
                    avg_tel_links
                ),
                actionable_recommendation: "Add prominent clickable tap-to-call phone links in the header, floating sticky bar, and throughout body content to capture mobile call leads.".to_string(),
            });
        }
    }

    let final_score = score.clamp(0, 100) as u32;

    GapAnalysisOutput {
        benchmarks,
        insights,
        opportunity_score: final_score,
        quick_wins,
        missing_content_outline,
    }
}

fn clean_heading(h: &str) -> String {
    let s = h.trim();
    // Strip leading numbers or bullets (e.g. "1. Car Accidents" -> "Car Accidents")
    let s = s.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == ' ');
    s.to_string()
}

