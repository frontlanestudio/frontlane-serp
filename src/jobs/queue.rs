use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};

use crate::core::engine::SearchEngine;
use crate::core::http_client::HttpClient;
use crate::jobs::types::{BatchRankProgress, BatchRankRequest, JobDetails, JobStatus};
use crate::rank::{probe_engine_rank, RankRequest};

#[derive(Clone)]
pub struct JobManager {
    jobs: Arc<RwLock<HashMap<String, JobDetails>>>,
    http_client: HttpClient,
}

impl JobManager {
    pub fn new(http_client: HttpClient) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            http_client,
        }
    }

    pub async fn submit_batch_rank(
        &self,
        req: BatchRankRequest,
        engine: Arc<dyn SearchEngine>,
    ) -> String {
        let job_id = format!("job_{}", uuid::Uuid::now_v7());
        let total = req.targets.len();

        let initial_details = JobDetails {
            job_id: job_id.clone(),
            status: JobStatus::Queued,
            progress: BatchRankProgress {
                total,
                completed: 0,
                failed: 0,
            },
            results: Vec::new(),
            errors: Vec::new(),
            created_at: Utc::now(),
            completed_at: None,
        };

        {
            let mut store = self.jobs.write().await;
            store.insert(job_id.clone(), initial_details);
        }

        let jobs_store = self.jobs.clone();
        let http_client = self.http_client.clone();
        let jid = job_id.clone();

        tokio::spawn(async move {
            {
                let mut store = jobs_store.write().await;
                if let Some(j) = store.get_mut(&jid) {
                    j.status = JobStatus::Processing;
                }
            }

            let concurrency = req.concurrency.clamp(1, 10);
            let sem = Arc::new(Semaphore::new(concurrency));
            let mut handles = Vec::new();

            for item in req.targets {
                let sem_clone = sem.clone();
                let engine_clone = engine.clone();
                let fallback = req.smart_full_fallback;
                let pag_limit = req.pagination_limit;

                let handle = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.unwrap();
                    let rank_req = RankRequest {
                        target: item.target,
                        q: item.query,
                        strategy: item.strategy,
                        last_rank: item.last_rank,
                        pagination_limit: pag_limit,
                        smart_full_fallback: fallback,
                        r#match: item.match_mode,
                        device: item.device,
                        region: "US".to_string(),
                        lang: "en".to_string(),
                    };
                    probe_engine_rank(engine_clone, &rank_req).await
                });
                handles.push(handle);
            }

            let mut results = Vec::new();
            let mut errors = Vec::new();

            for h in handles {
                match h.await {
                    Ok(Ok(res)) => {
                        results.push(res);
                        let mut store = jobs_store.write().await;
                        if let Some(j) = store.get_mut(&jid) {
                            j.progress.completed += 1;
                        }
                    }
                    Ok(Err(e)) => {
                        errors.push(e.to_string());
                        let mut store = jobs_store.write().await;
                        if let Some(j) = store.get_mut(&jid) {
                            j.progress.failed += 1;
                        }
                    }
                    Err(e) => {
                        errors.push(e.to_string());
                        let mut store = jobs_store.write().await;
                        if let Some(j) = store.get_mut(&jid) {
                            j.progress.failed += 1;
                        }
                    }
                }
            }

            let webhook_url = req.webhook_url.clone();
            let completed_details = {
                let mut store = jobs_store.write().await;
                if let Some(j) = store.get_mut(&jid) {
                    j.status = if errors.len() == j.progress.total && j.progress.total > 0 {
                        JobStatus::Failed
                    } else {
                        JobStatus::Completed
                    };
                    j.results = results.clone();
                    j.errors = errors.clone();
                    j.completed_at = Some(Utc::now());
                    Some(j.clone())
                } else {
                    None
                }
            };

            // Deliver outbound webhook if configured
            if let (Some(url), Some(details)) = (webhook_url, completed_details) {
                let _ = http_client
                    .post_json(&url, &serde_json::to_value(&details).unwrap_or_default())
                    .await;
            }
        });

        job_id
    }

    pub async fn get_job(&self, job_id: &str) -> Option<JobDetails> {
        let store = self.jobs.read().await;
        store.get(job_id).cloned()
    }
}
