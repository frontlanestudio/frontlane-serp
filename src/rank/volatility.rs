use serde::{Deserialize, Serialize};

/// Type of rank movement between two SERP observation points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RankMovement {
    Improved(u32),
    Declined(u32),
    Stable,
    NewEntry(u32),
    DroppedOut,
}

/// Delta calculation for a specific keyword and domain across SERP crawls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankDelta {
    pub keyword: String,
    pub domain: String,
    pub current_rank: Option<u32>,
    pub previous_rank: Option<u32>,
    pub delta: i32,
    pub movement: RankMovement,
}

impl RankDelta {
    /// Compute the rank delta and movement status between current and previous rankings.
    ///
    /// `delta` is positive when rank improved (e.g. rank 10 -> 3 = +7), negative when dropped
    /// (e.g. rank 3 -> 10 = -7), and 0 for stable rankings or unranked transitions.
    pub fn compute(
        keyword: &str,
        domain: &str,
        current: Option<u32>,
        previous: Option<u32>,
    ) -> Self {
        let (delta, movement) = match (current, previous) {
            (Some(c), Some(p)) => {
                let diff = p as i32 - c as i32;
                if diff > 0 {
                    (diff, RankMovement::Improved(diff as u32))
                } else if diff < 0 {
                    (diff, RankMovement::Declined((-diff) as u32))
                } else {
                    (0, RankMovement::Stable)
                }
            }
            (Some(c), None) => (0, RankMovement::NewEntry(c)),
            (None, Some(_p)) => (0, RankMovement::DroppedOut),
            (None, None) => (0, RankMovement::Stable),
        };

        Self {
            keyword: keyword.to_string(),
            domain: domain.to_string(),
            current_rank: current,
            previous_rank: previous,
            delta,
            movement,
        }
    }
}

/// Aggregate SERP Volatility Index reflecting rank turbulence across keyword sets.
///
/// Score ranges from 0.0 to 10.0, where scores > 6.0 indicate high volatility or Google core algorithm updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerpVolatilityIndex {
    pub score: f64,
    pub total_keywords: usize,
    pub improved_count: usize,
    pub declined_count: usize,
    pub stable_count: usize,
    pub churn_count: usize,
}

impl SerpVolatilityIndex {
    /// Calculate the SERP Volatility Index from a collection of rank deltas.
    pub fn calculate(deltas: &[RankDelta]) -> Self {
        let total = deltas.len();
        if total == 0 {
            return Self {
                score: 0.0,
                total_keywords: 0,
                improved_count: 0,
                declined_count: 0,
                stable_count: 0,
                churn_count: 0,
            };
        }

        let mut improved_count = 0;
        let mut declined_count = 0;
        let mut stable_count = 0;
        let mut churn_count = 0;
        let mut total_impact = 0.0;

        for delta in deltas {
            match delta.movement {
                RankMovement::Improved(d) => {
                    improved_count += 1;
                    total_impact += d as f64;
                }
                RankMovement::Declined(d) => {
                    declined_count += 1;
                    total_impact += d as f64;
                }
                RankMovement::Stable => {
                    stable_count += 1;
                }
                RankMovement::NewEntry(rank) => {
                    churn_count += 1;
                    let impact = if rank <= 10 {
                        10.0
                    } else if rank <= 20 {
                        7.5
                    } else {
                        5.0
                    };
                    total_impact += impact;
                }
                RankMovement::DroppedOut => {
                    churn_count += 1;
                    total_impact += 10.0;
                }
            }
        }

        let avg_impact = total_impact / total as f64;
        let raw_score = (avg_impact * 1.5).min(10.0);
        let score = (raw_score * 10.0).round() / 10.0;

        Self {
            score,
            total_keywords: total,
            improved_count,
            declined_count,
            stable_count,
            churn_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rank_delta_computation() {
        // Improved: previous 10, current 3 -> +7 delta
        let delta_improved = RankDelta::compute("personal injury lawyer", "calljacob.com", Some(3), Some(10));
        assert_eq!(delta_improved.delta, 7);
        assert_eq!(delta_improved.movement, RankMovement::Improved(7));

        // Declined: previous 2, current 8 -> -6 delta
        let delta_declined = RankDelta::compute("car accident attorney", "calljacob.com", Some(8), Some(2));
        assert_eq!(delta_declined.delta, -6);
        assert_eq!(delta_declined.movement, RankMovement::Declined(6));

        // Stable: previous 5, current 5 -> 0 delta
        let delta_stable = RankDelta::compute("wrongful death lawyer", "calljacob.com", Some(5), Some(5));
        assert_eq!(delta_stable.delta, 0);
        assert_eq!(delta_stable.movement, RankMovement::Stable);

        // NewEntry: previous None, current 4 -> NewEntry(4)
        let delta_new = RankDelta::compute("slip and fall lawyer", "calljacob.com", Some(4), None);
        assert_eq!(delta_new.delta, 0);
        assert_eq!(delta_new.movement, RankMovement::NewEntry(4));

        // DroppedOut: previous 7, current None -> DroppedOut
        let delta_dropped = RankDelta::compute("motorcycle accident attorney", "calljacob.com", None, Some(7));
        assert_eq!(delta_dropped.delta, 0);
        assert_eq!(delta_dropped.movement, RankMovement::DroppedOut);
    }

    #[test]
    fn test_serp_volatility_score() {
        // Low volatility scenario (quiet day, stable rankings)
        let quiet_deltas = vec![
            RankDelta::compute("kw1", "domain.com", Some(1), Some(1)),
            RankDelta::compute("kw2", "domain.com", Some(4), Some(4)),
            RankDelta::compute("kw3", "domain.com", Some(5), Some(4)), // -1 shift
            RankDelta::compute("kw4", "domain.com", Some(8), Some(9)), // +1 shift
            RankDelta::compute("kw5", "domain.com", Some(12), Some(12)),
        ];

        let quiet_index = SerpVolatilityIndex::calculate(&quiet_deltas);
        assert_eq!(quiet_index.total_keywords, 5);
        assert_eq!(quiet_index.stable_count, 3);
        assert_eq!(quiet_index.improved_count, 1);
        assert_eq!(quiet_index.declined_count, 1);
        assert_eq!(quiet_index.churn_count, 0);
        assert!(quiet_index.score < 3.0, "Score should be low on quiet days: got {}", quiet_index.score);

        // High volatility scenario (Google core algorithm update: large shifts & churn)
        let volatile_deltas = vec![
            RankDelta::compute("kw1", "domain.com", Some(2), Some(15)), // +13
            RankDelta::compute("kw2", "domain.com", Some(25), Some(3)),  // -22
            RankDelta::compute("kw3", "domain.com", None, Some(5)),      // DroppedOut
            RankDelta::compute("kw4", "domain.com", Some(1), None),      // NewEntry(1)
            RankDelta::compute("kw5", "domain.com", Some(18), Some(4)),  // -14
        ];

        let volatile_index = SerpVolatilityIndex::calculate(&volatile_deltas);
        assert_eq!(volatile_index.total_keywords, 5);
        assert_eq!(volatile_index.churn_count, 2);
        assert!(
            volatile_index.score > 6.0,
            "Volatility score should be > 6.0 for major core update turbulence: got {}",
            volatile_index.score
        );
    }
}
