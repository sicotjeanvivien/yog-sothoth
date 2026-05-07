use anyhow::Context;
use std::sync::Arc;
use yog_core::domain::PoolRepository;
use yog_persistence::{Database, PgPoolRepository};

use crate::bootstrap::Config;

/// Application-level dependencies shared across HTTP handlers.
///
/// Built once at startup, then handed (via `Arc`) to the route builder
/// which captures the relevant repositories in its closures. The
/// container itself doesn't need to live past router construction —
/// each handler holds the dependencies it actually uses.
pub(crate) struct AppState {
    /// The DB connection pool. Kept here so handlers that need raw
    /// access (transactions spanning multiple repos) can grab it.
    /// Most handlers should depend on a specific repository instead.
    _database: Database,

    /// Pool repository — read access for the api role.
    pub(crate) pool_repository: Arc<dyn PoolRepository>,
    // Future: SwapEventRepository, LiquidityEventRepository, …
    // Future: SignalService, AlertService, … (v0.2)
    // Future: UserService, AuthService, … (v0.3)
}

impl AppState {
    pub(crate) async fn build(config: Config) -> anyhow::Result<Self> {
        let database = Database::connect(config.database_url.expose())
            .await
            .context("failed to connect to database")?;

        let pool_repository: Arc<dyn PoolRepository> =
            Arc::new(PgPoolRepository::new(database.pool().clone()));

        Ok(Self {
            _database: database,
            pool_repository,
        })
    }
}
