use chrono::Utc;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};

use crate::core::engine::SearchEngine;
use crate::core::http_client::HttpClient;
use crate::core::network_guard::validate_public_url;
use crate::jobs::types::{BatchRankProgress, BatchRankRequest, JobDetails, JobStatus};
use crate::rank::{probe_engine_rank, RankRequest};

pub const DEFAULT_MAX_JOBS: u64 = 5_000;
pub const DEFAULT_JOB_TTL_HOURS: u64 = 24;

#[derive(Clone)]
pub struct JobManager {
    jobs: Cache<String, Arc<Mutex<JobDetails>>>,
    http_client: HttpClient,
}

impl JobManager {
    pub fn new(http_client: HttpClient) -> Self {
        let jobs = Cache::builder()
            .max_capacity(DEFAULT_MAX_JOBS)
            .time_to_live(Duration::from_secs(DEFAULT_JOB_TTL_HOURS * 3600))
            .build();

        Self { jobs, http_client }
    }

    pub async fn submit_batch_rank(
        &self,
        req: BatchRankRequest,
        engine: Arc<dyn SearchEngine>,
    ) -> String {
        let job_id = format!("job_{}", uuid::Uuid::now_v7());
        let total = req.targets.len();

        let initial_details = Arc::new(Mutex::new(JobDetails {
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
        }));

        self.jobs.insert(job_id.clone(), initial_details).await;

        let jobs_store = self.jobs.clone();
        let http_client = self.http_client.clone();
        let jid = job_id.clone();

        tokio::spawn(async move {
            if let Some(entry) = jobs_store.get(&jid).await {
                let mut store = entry.lock().await;
                store.status = JobStatus::Processing;
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
                        if let Some(entry) = jobs_store.get(&jid).await {
                            let mut store = entry.lock().await;
                            store.progress.completed += 1;
                        }
                    }
                    Ok(Err(e)) => {
                        errors.push(e.to_string());
                        if let Some(entry) = jobs_store.get(&jid).await {
                            let mut store = entry.lock().await;
                            store.progress.failed += 1;
                        }
                    }
                    Err(e) => {
                        errors.push(e.to_string());
                        if let Some(entry) = jobs_store.get(&jid).await {
                            let mut store = entry.lock().await;
                            store.progress.failed += 1;
                        }
                    }
                }
            }

            let webhook_url = req.webhook_url.clone();
            let completed_details = if let Some(entry) = jobs_store.get(&jid).await {
                let mut store = entry.lock().await;
                store.status = if errors.len() == store.progress.total && store.progress.total > 0 {
                    JobStatus::Failed
                } else {
                    JobStatus::Completed
                };
                store.results = results.clone();
                store.errors = errors.clone();
                store.completed_at = Some(Utc::now());
                Some(store.clone())
            } else {
                None
            };

            // Deliver outbound webhook if configured (guarded against SSRF)
            if let (Some(url), Some(details)) = (webhook_url, completed_details) {
                if validate_public_url(&url).await.is_ok() {
                    let _ = http_client
                        .post_json(&url, &serde_json::to_value(&details).unwrap_or_default())
                        .await;
                } else {
                    tracing::warn!(url = %url, "blocked unverified or non-public webhook URL in background job");
                }
            }
        });

        job_id
    }

    pub async fn get_job(&self, job_id: &str) -> Option<JobDetails> {
        if let Some(entry) = self.jobs.get(job_id).await {
            let store = entry.lock().await;
            Some(store.clone())
        } else {
            None
        }
    }
}
