use frontlane_serp::core::http_client::HttpClient;
use frontlane_serp::engines::Google;
use frontlane_serp::jobs::{BatchRankItem, BatchRankRequest, JobManager};
use frontlane_serp::rank::{DeviceType, DomainMatchMode, RankStrategy};
use std::sync::Arc;

#[tokio::test]
async fn test_job_manager_lifecycle() {
    let http_client = HttpClient::new(None, true, 10).unwrap();
    let manager = JobManager::new(http_client.clone());
    let engine = Arc::new(Google::new(http_client));

    let req = BatchRankRequest {
        targets: vec![BatchRankItem {
            target: "example.com".to_string(),
            query: "sample query".to_string(),
            last_rank: 0,
            strategy: RankStrategy::Basic,
            device: DeviceType::Desktop,
            match_mode: DomainMatchMode::Subdomain,
        }],
        engine: "google".to_string(),
        concurrency: 2,
        webhook_url: None,
        smart_full_fallback: false,
        pagination_limit: 1,
    };

    let job_id = manager.submit_batch_rank(req, engine).await;
    assert!(job_id.starts_with("job_"));

    let details = manager.get_job(&job_id).await;
    assert!(details.is_some());
    let d = details.unwrap();
    assert_eq!(d.job_id, job_id);
    assert_eq!(d.progress.total, 1);
}
