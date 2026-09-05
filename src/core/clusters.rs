use std::collections::HashMap;
use crate::core::response_builder::{build_cluster_id, normalize_url};
use crate::core::types::{Cluster, ClusterOccurrence, ResultItem};

pub fn build_clusters(results: &[ResultItem], engines_queried: usize) -> Vec<Cluster> {
    let engines_queried = if engines_queried == 0 { 1 } else { engines_queried };

    struct ClusterAccum {
        occurrences: Vec<ClusterOccurrence>,
        score_sum: f64,
        best_rank: usize,
        title: String,
        canonical_url: String,
        domain: String,
    }

    let mut by_url: HashMap<String, ClusterAccum> = HashMap::new();
    let mut url_order: Vec<String> = Vec::new();

    for r in results {
        let norm = normalize_url(&r.url);
        if norm.is_empty() {
            continue;
        }

        let rank = if r.rank == 0 { 1 } else { r.rank };
        let entry = by_url.entry(norm.clone()).or_insert_with(|| {
            url_order.push(norm.clone());
            ClusterAccum {
                occurrences: Vec::new(),
                score_sum: 0.0,
                best_rank: r.rank,
                title: r.title.clone(),
                canonical_url: r.url.clone(),
                domain: r.domain.clone(),
            }
        });

        entry.score_sum += 1.0 / (rank as f64);
        entry.occurrences.push(ClusterOccurrence {
            engine: r.engine.clone(),
            rank: r.rank,
            result_id: r.id.clone(),
        });

        if r.rank > 0 && (entry.best_rank == 0 || r.rank < entry.best_rank) {
            entry.best_rank = r.rank;
            entry.title = r.title.clone();
            entry.canonical_url = r.url.clone();
            entry.domain = r.domain.clone();
        }
    }

    let mut clusters: Vec<Cluster> = url_order
        .into_iter()
        .filter_map(|norm| {
            let acc = by_url.remove(&norm)?;
            let mut score = acc.score_sum / (engines_queried as f64);
            if score > 1.0 {
                score = 1.0;
            }
            let rounded_score = (score * 10000.0).round() / 10000.0;
            let id = build_cluster_id(&norm);
            let engines_count = acc.occurrences.len();

            Some(Cluster {
                id,
                canonical_url: acc.canonical_url,
                domain: acc.domain,
                title: acc.title,
                occurrences: acc.occurrences,
                engines_count,
                best_rank: acc.best_rank,
                score: rounded_score,
            })
        })
        .collect();

    clusters.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.best_rank.cmp(&b.best_rank))
    });

    clusters
}
