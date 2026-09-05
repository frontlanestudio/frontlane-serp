use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use chrono::Utc;
use tokio::task::JoinSet;

use crate::core::clusters::build_clusters;
use crate::core::engine::SearchEngine;
use crate::core::error::Result;
use crate::core::response_builder::{enrich_image_result, enrich_result, normalize_url};
use crate::core::types::{
    EngineErrorDetail, Envelope, ImageEnvelope, ImageResult, Pagination, Query,
    ResultItem, SearchResult, SerpFeature,
};
use crate::extract::Extractor;

#[derive(Clone)]
pub struct MegaSearcher {
    engines: HashMap<String, Arc<dyn SearchEngine>>,
    extractor: Option<Extractor>,
}

impl MegaSearcher {
    pub fn new(engines: Vec<Arc<dyn SearchEngine>>, extractor: Option<Extractor>) -> Self {
        let mut map = HashMap::new();
        for e in engines {
            map.insert(e.name().to_string(), e);
        }
        Self { engines: map, extractor }
    }

    pub async fn search(&self, query: &Query, engines_requested: &[String], mode: &str) -> Result<Envelope> {
        let started_at = Utc::now();
        let start_time = Instant::now();

        let mut available_engines = Vec::new();
        for name in engines_requested {
            let lower = name.to_lowercase();
            let normalized = match lower.as_str() {
                "ddg" | "duck" => "duckduckgo",
                n => n,
            };
            if let Some(engine) = self.engines.get(normalized) {
                available_engines.push((normalized.to_string(), engine.clone()));
            }
        }

        let mut join_set = JoinSet::new();
        for (name, engine) in available_engines {
            let q = query.clone();
            join_set.spawn(async move {
                let res = engine.search(&q).await;
                (name, res)
            });
        }

        let mut all_results: Vec<(String, Vec<SearchResult>)> = Vec::new();
        let mut engines_failed: Vec<String> = Vec::new();
        let mut engine_errors: Vec<EngineErrorDetail> = Vec::new();
        let mut engines_responded: Vec<String> = Vec::new();

        if mode == "any" {
            while let Some(res) = join_set.join_next().await {
                if let Ok((name, search_res)) = res {
                    match search_res {
                        Ok(items) if !items.is_empty() => {
                            engines_responded.push(name.clone());
                            all_results.push((name, items));
                            join_set.abort_all();
                            break;
                        }
                        Ok(_) => {
                            engines_responded.push(name.clone());
                        }
                        Err(e) => {
                            engines_failed.push(name.clone());
                            engine_errors.push(EngineErrorDetail {
                                engine: name,
                                error: e.to_string(),
                                message: None,
                            });
                        }
                    }
                }
            }
        } else {
            while let Some(res) = join_set.join_next().await {
                if let Ok((name, search_res)) = res {
                    match search_res {
                        Ok(items) => {
                            engines_responded.push(name.clone());
                            all_results.push((name, items));
                        }
                        Err(e) => {
                            engines_failed.push(name.clone());
                            engine_errors.push(EngineErrorDetail {
                                engine: name,
                                error: e.to_string(),
                                message: None,
                            });
                        }
                    }
                }
            }
        }

        // Flatten and enrich results
        let mut enriched_results: Vec<ResultItem> = Vec::new();
        let mut serp_features: Vec<SerpFeature> = Vec::new();

        for (engine_name, items) in all_results {
            for raw in items {
                for feat in &raw.features {
                    serp_features.push(feat.clone());
                }
                let enriched = enrich_result(raw, &engine_name, query.start);
                enriched_results.push(enriched);
            }
        }

        // Build clusters across all engine results before deduplication
        let clusters = build_clusters(&enriched_results, engines_requested.len());

        // Deduplicate across engines by normalized URL, keeping best rank
        let deduped = deduplicate_mega_results(enriched_results);

        // Extraction if requested
        let mut final_results = deduped;
        if query.extract {
            if let Some(ref ext) = self.extractor {
                let extract_count = query.extract_top.min(final_results.len());
                for item in final_results.iter_mut().take(extract_count) {
                    if let Ok(content) = ext.extract(&item.url, true).await {
                        item.extracted = Some(content);
                    }
                }
            }
        }

        let took_ms = start_time.elapsed().as_millis() as i64;
        let mut env = Envelope::new(query, uuid::Uuid::now_v7().to_string(), started_at, engines_requested.to_vec());
        env.meta.took_ms = took_ms;
        env.meta.engines_responded = engines_responded;
        env.meta.engines_failed = engines_failed;
        env.meta.engine_errors = engine_errors;
        env.results = final_results;
        env.serp_features = serp_features;
        env.clusters = Some(clusters);
        env.pagination = Pagination {
            page: (query.start / 10) + 1,
            has_more: env.results.len() >= query.limit,
            next_start: query.start + query.limit,
        };

        Ok(env)
    }

    pub async fn search_image(&self, query: &Query, engines_requested: &[String]) -> Result<ImageEnvelope> {
        let started_at = Utc::now();
        let start_time = Instant::now();

        let mut available_engines = Vec::new();
        for name in engines_requested {
            if let Some(engine) = self.engines.get(name) {
                available_engines.push((name.clone(), engine.clone()));
            }
        }

        let mut join_set = JoinSet::new();
        for (name, engine) in available_engines {
            let q = query.clone();
            join_set.spawn(async move {
                let res = engine.search_image(&q).await;
                (name, res)
            });
        }

        let mut all_results: Vec<ImageResult> = Vec::new();
        let mut engines_failed = Vec::new();
        let mut engines_responded = Vec::new();

        while let Some(res) = join_set.join_next().await {
            if let Ok((name, search_res)) = res {
                match search_res {
                    Ok(items) => {
                        engines_responded.push(name.clone());
                        for item in items {
                            all_results.push(enrich_image_result(item, &name));
                        }
                    }
                    Err(_) => {
                        engines_failed.push(name);
                    }
                }
            }
        }

        let took_ms = start_time.elapsed().as_millis() as i64;
        let env = ImageEnvelope {
            query: crate::core::types::QueryEcho {
                text: query.text.clone(),
                lang: query.lang_code.clone(),
                region: query.region.clone(),
                engines_requested: engines_requested.to_vec(),
            },
            meta: crate::core::types::ResponseMeta {
                request_id: uuid::Uuid::now_v7().to_string(),
                requested_at: started_at.to_rfc3339(),
                took_ms,
                engines_responded,
                engines_failed,
                engine_errors: Vec::new(),
                version: crate::core::types::API_VERSION.to_string(),
            },
            results: all_results,
            pagination: Pagination {
                page: 1,
                has_more: false,
                next_start: 0,
            },
        };

        Ok(env)
    }
}

fn deduplicate_mega_results(results: Vec<ResultItem>) -> Vec<ResultItem> {
    let mut map: HashMap<String, ResultItem> = HashMap::new();
    let mut order = Vec::new();

    for r in results {
        let norm = normalize_url(&r.url);
        if norm.is_empty() {
            continue;
        }
        if let Some(existing) = map.get_mut(&norm) {
            if r.rank > 0 && (existing.rank == 0 || r.rank < existing.rank) {
                *existing = r;
            }
        } else {
            order.push(norm.clone());
            map.insert(norm, r);
        }
    }

    let mut deduped: Vec<ResultItem> = order.into_iter().filter_map(|k| map.remove(&k)).collect();
    deduped.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then_with(|| a.engine.cmp(&b.engine))
            .then_with(|| a.url.cmp(&b.url))
    });

    deduped
}
