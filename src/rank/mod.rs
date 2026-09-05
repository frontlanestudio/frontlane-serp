pub mod matcher;
pub mod prober;
pub mod types;

pub use matcher::{matches_target, normalize_domain_or_url};
pub use prober::{calculate_pages_to_probe, probe_engine_rank};
pub use types::{DeviceType, DomainMatchMode, RankRequest, RankResponse, RankStrategy};
