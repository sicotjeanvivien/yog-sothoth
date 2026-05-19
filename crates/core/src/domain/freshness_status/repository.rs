//! Event freshness repository trait.
//!
//! A minimal repository whose single responsibility is to answer
//! "when was the most recent event indexed". It does not belong on
//! any CRUD repository (swap, liquidity) — picking one arbitrarily
//! would be a poor fit — so it stands on its own.
//!
//! Placed in `domain` alongside the other repository traits, matching
//! the crate's convention.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::RepositoryResult;

/// Reads the ingestion freshness signal.
#[async_trait]
pub trait EventFreshnessRepository: Send + Sync {
    /// Timestamp of the most recent indexed event.
    ///
    /// Defined as the maximum `timestamp` across `swap_events` and
    /// `liquidity_events` — the two tables that reflect actual pool
    /// activity. `None` when neither table holds any row (empty
    /// database).
    async fn last_event_at(&self) -> RepositoryResult<Option<DateTime<Utc>>>;
}
