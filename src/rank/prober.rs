use std::sync::Arc;
use std::time::Instant;

use crate::core::engine::SearchEngine;
use crate::core::response_builder::enrich_result;
use crate::core::types::{Query, ResultType, SerpFeature};
use crate::rank::matcher::matches_target;
use crate::rank::types::{RankRequest, RankResponse, RankStrategy};

pub fn calculate_pages_to_probe(
    strategy: RankStrategy,
    last_rank: usize,
    limit: usize,
) -> Vec<usize> {
    let max_pages = limit.clamp(1, 10);
    match strategy {
        RankStrategy::Basic => vec![1],
        RankStrategy::Custom => (1..=max_pages).collect(),
        RankStrategy::Smart => {
            if last_rank == 0 {
                vec![1]
            } else {
                let target_page = last_rank.div_ceil(10);
                let mut pages = Vec::new();
                if target_page > 1 {
                    pages.push(target_page - 1);
                }
                pages.push(target_page);
                if target_page < max_pages {
                    pages.push(target_page + 1);
                }
                pages.retain(|&p| p <= max_pages);
                pages.dedup();
                if pages.is_empty() {
                    vec![1]
                } else {
                    pages
                }
            }
        }
    }
}

fn process_page_items(
    items: Vec<crate::core::types::ResultItem>,
    req: &RankRequest,
    ranked_hit: &mut Option<crate::core::types::ResultItem>,
    serp_results: &mut Vec<crate::rank::types::SerpRankResultItem>,
) -> bool {
    let mut target_found = false;
    for item in items {
        let is_target = matches_target(&item.url, &req.target, req.r#match);
        let rank = if item.rank > 0 {
            item.rank
        } else if let Some(ref pos) = item.position {
            pos.absolute
        } else {
            serp_results.len() + 1
        };

        if is_target && ranked_hit.is_none() {
            *ranked_hit = Some(item.clone());
            target_found = true;
        }

        let is_dir = crate::core::domain::is_directory_domain(&item.domain)
            || item.domain_info.as_ref().map(|d| d.category.as_str()) == Some("directory");
        let domain_category = item.domain_info.as_ref().map(|d| d.category.clone());

        serp_results.push(crate::rank::types::SerpRankResultItem {
            rank,
            url: item.url,
            title: item.title,
            snippet: if item.snippet.is_empty() {
                None
            } else {
                Some(item.snippet)
            },
            domain: if item.domain.is_empty() {
                None
            } else {
                Some(item.domain)
            },
            is_target,
            is_directory: is_dir,
            domain_category,
        });
    }
    target_found
}

fn feature_has_target_link(feature: &SerpFeature, req: &RankRequest) -> bool {
    feature
        .links
        .iter()
        .filter_map(|l| l.url.as_deref())
        .any(|u| matches_target(u, &req.target, req.r#match))
}

fn extract_feature_citations(features: &[SerpFeature], req: &RankRequest) -> Vec<String> {
    let mut feature_citations = Vec::new();
    for feature in features {
        match feature.feature_type {
            ResultType::AnswerBox if feature_has_target_link(feature, req) => {
                feature_citations.push("AnswerBox".to_string());
            }
            ResultType::KnowledgePanel if feature_has_target_link(feature, req) => {
                feature_citations.push("KnowledgePanel".to_string());
            }
            ResultType::PeopleAlsoAsk | ResultType::RelatedQuestions => {
                let has_match = feature
                    .items
                    .iter()
                    .filter_map(|it| it.link.as_deref())
                    .any(|link| matches_target(link, &req.target, req.r#match));
                if has_match {
                    feature_citations.push("PeopleAlsoAsk".to_string());
                }
            }
            _ => {}
        }
    }
    feature_citations.dedup();
    feature_citations
}

fn compute_directory_metrics(
    serp_results: &[crate::rank::types::SerpRankResultItem],
) -> (usize, f64, Vec<String>) {
    let mut directory_count = 0;
    let mut ranking_directories = Vec::new();
    for res in serp_results {
        if res.is_directory {
            directory_count += 1;
            let dir_name = res.domain.as_deref().unwrap_or(&res.url);
            ranking_directories.push(format!("{} (#{})", dir_name, res.rank));
        }
    }
    let directory_share_pct = if !serp_results.is_empty() {
        (directory_count as f64 / serp_results.len() as f64) * 100.0
    } else {
        0.0
    };
    (directory_count, directory_share_pct, ranking_directories)
}

fn resolve_ranked_hit(
    ranked_hit: Option<crate::core::types::ResultItem>,
) -> (bool, Option<usize>, Option<String>, Option<String>) {
    match ranked_hit {
        Some(hit) => {
            let computed_rank = if hit.rank > 0 {
                hit.rank
            } else if let Some(ref pos) = hit.position {
                pos.absolute
            } else {
                1
            };
            (true, Some(computed_rank), Some(hit.url), Some(hit.title))
        }
        None => (false, None, None, None),
    }
}

async fn probe_single_page(
    engine: &Arc<dyn SearchEngine>,
    req: &RankRequest,
    page: usize,
) -> Result<
    (Vec<crate::core::types::ResultItem>, Vec<SerpFeature>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let start = (page - 1) * 10;
    let q = Query {
        text: req.q.clone(),
        lang_code: req.lang.clone(),
        region: req.region.clone(),
        date_interval: String::new(),
        filetype: String::new(),
        site: String::new(),
        limit: 10,
        start,
        filter: true,
        features: true,
        extract: false,
        extract_top: 0,
        extract_mode: "auto".to_string(),
        extract_min_runes: 0,
        proxy_url: None,
        proxy_country: None,
        proxy_class: None,
        proxy_provider: None,
        proxy_session_id: None,
        proxy_override: None,
        insecure: true,
        guard_private_networks: false,
    };

    let raw_results = engine.search(&q).await?;
    let mut enriched = Vec::new();
    let mut features = Vec::new();

    for raw in raw_results {
        for f in &raw.features {
            features.push(f.clone());
        }
        let item = enrich_result(raw, engine.name(), start);
        enriched.push(item);
    }

    Ok((enriched, features))
}

pub async fn probe_engine_rank(
    engine: Arc<dyn SearchEngine>,
    req: &RankRequest,
) -> Result<RankResponse, Box<dyn std::error::Error + Send + Sync>> {
    let started = Instant::now();
    let initial_pages = calculate_pages_to_probe(req.strategy, req.last_rank, req.pagination_limit);
    let mut pages_scraped = Vec::new();
    let mut all_serp_features = Vec::new();
    let mut serp_results = Vec::new();
    let mut ranked_hit = None;

    for page in &initial_pages {
        pages_scraped.push(*page);
        let (items, features) = probe_single_page(&engine, req, *page).await?;
        all_serp_features.extend(features);

        if process_page_items(items, req, &mut ranked_hit, &mut serp_results) {
            break;
        }
    }

    // If not found in smart initial pages and full fallback requested
    if ranked_hit.is_none() && req.smart_full_fallback && req.strategy == RankStrategy::Smart {
        let max_pages = req.pagination_limit.clamp(1, 10);
        for page in 1..=max_pages {
            if !pages_scraped.contains(&page) {
                pages_scraped.push(page);
                let (items, features) = probe_single_page(&engine, req, page).await?;
                all_serp_features.extend(features);

                if process_page_items(items, req, &mut ranked_hit, &mut serp_results) {
                    break;
                }
            }
        }
    }

    let feature_citations = extract_feature_citations(&all_serp_features, req);
    let (directory_count, directory_share_pct, ranking_directories) =
        compute_directory_metrics(&serp_results);
    let took_ms = started.elapsed().as_millis() as i64;
    let (ranked, rank, url, title) = resolve_ranked_hit(ranked_hit);

    Ok(RankResponse {
        target: req.target.clone(),
        query: req.q.clone(),
        engine: engine.name().to_string(),
        device: format!("{:?}", req.device).to_lowercase(),
        ranked,
        rank,
        url,
        title,
        serp_features: all_serp_features,
        feature_citations,
        pages_scraped,
        took_ms,
        directory_count,
        directory_share_pct,
        ranking_directories,
        serp_results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_pages_to_probe() {
        assert_eq!(calculate_pages_to_probe(RankStrategy::Basic, 0, 5), vec![1]);
        assert_eq!(
            calculate_pages_to_probe(RankStrategy::Custom, 0, 3),
            vec![1, 2, 3]
        );
        assert_eq!(calculate_pages_to_probe(RankStrategy::Smart, 0, 5), vec![1]);
        assert_eq!(
            calculate_pages_to_probe(RankStrategy::Smart, 5, 5),
            vec![1, 2]
        ); // page 1 -> neighbors [1, 2]
        assert_eq!(
            calculate_pages_to_probe(RankStrategy::Smart, 24, 5),
            vec![2, 3, 4]
        ); // pos 24 -> page 3 -> [2, 3, 4]
        assert_eq!(
            calculate_pages_to_probe(RankStrategy::Smart, 45, 5),
            vec![4, 5]
        ); // pos 45 -> page 5 -> [4, 5]
    }
}
