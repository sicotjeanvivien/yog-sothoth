//! Postgres persistence layer for yog-sothoth.
//!
//! This crate provides concrete implementations of the repository traits
//! defined in `yog-core`. It is consumed by both the indexer (write-heavy,
//! event ingestion) and the api (read-heavy, dashboard queries).
//!
//! Each consumer process instantiates its own connection pool with its own
//! database role. The crate itself does NOT own the pool — it is passed in
//! at construction time. This allows different processes to operate under
//! different Postgres roles (least privilege), while sharing the same SQL
//! and schema definitions.

mod database;
mod repositories;
mod repository_utils;

pub use database::Database;
pub use repositories::{
    PgClaimPositionFeeEventRepository, PgClaimRewardEventRepository, PgEventFreshnessRepository,
    PgLiquidityEventRepository, PgNetworkStatusRepository, PgPoolCurrentStateRepository,
    PgPoolRepository, PgSwapEventRepository, PgWatchedPoolRepository,
};

/// Re-export `sqlx::PgPool` so consumers don't need to depend on sqlx directly
/// just to type their dependency-injection wiring.
pub use sqlx::PgPool;
