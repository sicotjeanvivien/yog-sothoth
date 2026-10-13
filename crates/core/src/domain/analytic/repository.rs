//! The analytic tables' repository trait — yog-analytic is its only writer.

use async_trait::async_trait;

use super::{AnalyticProgress, CycleOutcome, CycleWrite};
use crate::RepositoryResult;

/// Reads how far the analytic tables go, and writes one cycle into them.
#[async_trait]
pub trait AnalyticRepository: Send + Sync {
    async fn progress(&self) -> RepositoryResult<AnalyticProgress>;

    /// All of `cycle` or nothing. A frozen hour is left as it is, whatever
    /// range the cycle names.
    async fn write_cycle(&self, cycle: &CycleWrite) -> RepositoryResult<CycleOutcome>;
}
