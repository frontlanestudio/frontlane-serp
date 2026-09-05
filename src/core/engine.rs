use async_trait::async_trait;
use std::collections::HashSet;
use crate::core::error::Result;
use crate::core::types::{Query, SearchResult};

#[async_trait]
pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_initialized(&self) -> bool {
        true
    }
    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>>;
    async fn search_image(&self, query: &Query) -> Result<Vec<SearchResult>>;
}

pub fn count_organic_results(results: &[SearchResult]) -> usize {
    results.iter().filter(|r| !r.ad).count()
}

pub fn limit_organic_results(results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    if limit == 0 {
        return results;
    }
    let mut out = Vec::with_capacity(results.len());
    let mut organic_count = 0;
    for r in results {
        if r.ad {
            out.push(r);
            continue;
        }
        if organic_count >= limit {
            continue;
        }
        organic_count += 1;
        out.push(r);
    }
    out
}

pub fn deduplicate_results(mut results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for r in results.drain(..) {
        if r.url.is_empty() {
            continue;
        }
        let key = format!("{}\0{}", if r.ad { "ad" } else { "organic" }, r.url);
        if seen.insert(key) {
            deduped.push(r);
        }
    }

    deduped.sort_by(|a, b| {
        let pos_a = if a.absolute_rank > 0 { a.absolute_rank } else { a.rank.abs() };
        let pos_b = if b.absolute_rank > 0 { b.absolute_rank } else { b.rank.abs() };
        pos_a
            .cmp(&pos_b)
            .then_with(|| a.ad.cmp(&b.ad))
            .then_with(|| a.rank.cmp(&b.rank))
            .then_with(|| a.url.cmp(&b.url))
    });

    deduped
}
