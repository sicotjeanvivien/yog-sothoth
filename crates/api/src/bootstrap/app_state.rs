use std::sync::Arc;
use yog_core::domain::PoolRepository;
use yog_persistence::{Database, PgPoolRepository};

use crate::bootstrap::Config;
use anyhow::Context;

/// Application-level dependencies shared across HTTP handlers.
///
/// `Clone` is cheap because every field is wrapped in `Arc` — axum
/// requires `Clone + Send + Sync + 'static` for its `State` extractor.
///
/// The DB pool is held inside each repository (via `PgPool`, itself an
/// `Arc` internally), so the `Database` wrapper does not need to live
/// on `AppState` after construction.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool_repository: Arc<dyn PoolRepository>,
    // Future: SwapEventRepository, LiquidityEventRepository, …
}

impl AppState {
    pub(crate) async fn build(config: Config) -> anyhow::Result<Self> {
        let database = Database::connect(config.database_url.expose())
            .await
            .context("failed to connect to database")?;

        let pool_repository: Arc<dyn PoolRepository> =
            Arc::new(PgPoolRepository::new(database.pool().clone()));

        Ok(Self { pool_repository })
    }
}
