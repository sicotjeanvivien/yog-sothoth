//! The analytic tables' repository trait — yog-analytic is its only writer.

use async_trait::async_trait;

use chrono::{DateTime, Utc};

use super::{AnalyticProgress, CycleOutcome, CycleWrite, TvlMeasure};
use crate::RepositoryResult;

/// Reads how far the analytic tables go, writes one cycle into them, and measures the TVL.
#[async_trait]
pub trait AnalyticRepository: Send + Sync {
    async fn progress(&self) -> RepositoryResult<AnalyticProgress>;

    /// All of `cycle` or nothing. A frozen hour is left as it is, whatever
    /// range the cycle names.
    async fn write_cycle(&self, cycle: &CycleWrite) -> RepositoryResult<CycleOutcome>;

    /// Every protocol's current TVL, written to `hour` unless it already holds one; apart from
    /// any cycle, so that a failed measure costs no freeze.
    async fn measure_tvl(
        &self,
        hour: DateTime<Utc>,
        at: DateTime<Utc>,
    ) -> RepositoryResult<TvlMeasure>;
}
