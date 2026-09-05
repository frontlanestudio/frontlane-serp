use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::rank::{DeviceType, DomainMatchMode, RankResponse, RankStrategy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BatchRankItem {
    pub target: String,
    pub query: String,
    #[serde(default)]
    pub last_rank: usize,
    #[serde(default)]
    pub strategy: RankStrategy,
    #[serde(default)]
    pub device: DeviceType,
    #[serde(default)]
    pub match_mode: DomainMatchMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BatchRankRequest {
    pub targets: Vec<BatchRankItem>,
    #[serde(default = "default_engine")]
    pub engine: String,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub smart_full_fallback: bool,
    #[serde(default = "default_pagination_limit")]
    pub pagination_limit: usize,
}

fn default_engine() -> String {
    "google".to_string()
}

fn default_concurrency() -> usize {
    3
}

fn default_pagination_limit() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRankProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetails {
    pub job_id: String,
    pub status: JobStatus,
    pub progress: BatchRankProgress,
    pub results: Vec<RankResponse>,
    pub errors: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
