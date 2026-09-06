use crate::audit::gap_analyzer::analyze_gaps;
use crate::audit::parser::parse_page_audit;
use crate::audit::types::{KeywordAuditReport, PageAuditResult};
use crate::core::engine::SearchEngine;
use crate::core::http_client::HttpClient;
use crate::rank::{
    matches_target, probe_engine_rank, DeviceType, DomainMatchMode, RankRequest, RankStrategy,
};
use std::sync::Arc;

pub async fn run_audit(
    engine: Arc<dyn SearchEngine>,
    http_client: &HttpClient,
    target_domain: &str,
    target_url_override: Option<&str>,
    keyword: &str,
    competitor_limit: usize,
) -> Result<KeywordAuditReport, Box<dyn std::error::Error + Send + Sync>> {
    let rank_req = RankRequest {
        target: target_domain.to_string(),
        q: keyword.to_string(),
        strategy: RankStrategy::Smart,
        last_rank: 0,
        pagination_limit: 2,
        smart_full_fallback: false,
        r#match: DomainMatchMode::Subdomain,
        device: DeviceType::Desktop,
        region: "US".to_string(),
        lang: "en".to_string(),
    };

    let rank_resp = probe_engine_rank(engine.clone(), &rank_req).await?;

    let target_url = if let Some(u) = target_url_override {
        u.to_string()
    } else if let Some(ref u) = rank_resp.url {
        u.clone()
    } else {
        if target_domain.starts_with("http://") || target_domain.starts_with("https://") {
            target_domain.to_string()
        } else {
            format!("https://{}/", target_domain.trim_start_matches("www."))
        }
    };

    // Filter competitor results from SERP
    let competitors_to_audit: Vec<(usize, String)> = rank_resp
        .serp_results
        .iter()
        .filter(|r| !r.is_target && !matches_target(&r.url, target_domain, DomainMatchMode::Subdomain))
        .take(competitor_limit)
        .map(|r| (r.rank, r.url.clone()))
        .collect();

    // Audit target page
    let target_audit = audit_single_url(
        http_client,
        &target_url,
        keyword,
        "Target".to_string(),
        rank_resp.rank,
        true,
    )
    .await;

    // Audit competitors concurrently
    let mut comp_futures = Vec::new();
    for (rank_num, comp_url) in competitors_to_audit {
        let client = http_client.clone();
        let kw = keyword.to_string();
        comp_futures.push(tokio::spawn(async move {
            audit_single_url(
                &client,
                &comp_url,
                &kw,
                format!("#{}", rank_num),
                Some(rank_num),
                false,
            )
            .await
        }));
    }

    let mut competitor_audits = Vec::new();
    for fut in comp_futures {
        if let Ok(res) = fut.await {
            competitor_audits.push(res);
        }
    }

    competitor_audits.sort_by_key(|c| c.rank_num.unwrap_or(999));

    let has_local_pack = rank_resp.serp_features.iter().any(|f| matches!(f.feature_type, crate::core::types::ResultType::Local));
    let has_paa = rank_resp.serp_features.iter().any(|f| matches!(f.feature_type, crate::core::types::ResultType::PeopleAlsoAsk | crate::core::types::ResultType::RelatedQuestions));

    let gap_out = analyze_gaps(
        keyword,
        Some(&target_audit),
        &competitor_audits,
        has_local_pack,
        has_paa,
    );

    Ok(KeywordAuditReport {
        keyword: keyword.to_string(),
        target_domain: target_domain.to_string(),
        target_url: Some(target_url),
        engine: engine.name().to_string(),
        target_rank: rank_resp.rank,
        target_audit: Some(target_audit),
        competitor_audits,
        benchmarks: gap_out.benchmarks,
        insights: gap_out.insights,
        opportunity_score: gap_out.opportunity_score,
        quick_wins: gap_out.quick_wins,
        missing_content_outline: gap_out.missing_content_outline,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

async fn audit_single_url(
    client: &HttpClient,
    url: &str,
    keyword: &str,
    rank_label: String,
    rank_num: Option<usize>,
    is_target: bool,
) -> PageAuditResult {
    match client
        .fetch_raw_response(url, Some("en"), None, None, None)
        .await
    {
        Ok((status, html)) => parse_page_audit(
            &html,
            url,
            keyword,
            status.as_u16(),
            rank_label,
            rank_num,
            is_target,
        ),
        Err(e) => {
            let domain = url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
                .unwrap_or_default();
            PageAuditResult {
                rank_label,
                rank_num,
                is_target,
                url: url.to_string(),
                domain,
                status: 0,
                title: String::new(),
                title_length: 0,
                title_exact_match: false,
                title_starts_with_kw: false,
                meta_description: String::new(),
                meta_description_length: 0,
                meta_exact_match: false,
                h1: Vec::new(),
                h1_exact_match: false,
                h2_count: 0,
                h2_headings: Vec::new(),
                h3_headings: Vec::new(),
                questions_found: Vec::new(),
                word_count: 0,
                exact_keyword_count: 0,
                keyword_density_pct: 0.0,
                slug_has_exact_kw: false,
                slug_has_geo: false,
                is_dedicated_page: false,
                is_directory_aggregator: false,
                canonical_url: None,
                is_self_canonical: false,
                is_noindex: false,
                tel_links_count: 0,
                form_count: 0,
                schema_types: Vec::new(),
                has_local_business_schema: false,
                has_review_rating_schema: false,
                has_faq_schema: false,
                review_count: None,
                rating_value: None,
                error: Some(format!("{}", e)),
            }
        }
    }
}
