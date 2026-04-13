use std::sync::Arc;

use yog_core::CoreResult;

use crate::{
    domain::{WatchedPool, WatchedPoolRepository},
    infra::{PgWatchedPoolRepository, RpcListener},
};

pub struct WatchedPoolService {
    listener: Arc<RpcListener>,
    repository: Arc<PgWatchedPoolRepository>,
}

impl WatchedPoolService {
    pub(crate) fn new(
        listener: Arc<RpcListener>,
        repository: Arc<PgWatchedPoolRepository>,
    ) -> Self {
        Self {
            listener,
            repository,
        }
    }

    pub async fn watch(&self, pool: WatchedPool) -> CoreResult<()> {
        self.repository.add(&pool).await?;
        self.listener.watch(pool.address).await;
        Ok(())
    }

    pub async fn unwatch(&self, pool: WatchedPool) -> CoreResult<()> {
        self.repository.remove(&pool).await?;
        self.listener.unwatch(pool.address).await;
        Ok(())
    }

    // Phase 3 — au démarrage, réabonner les pools déjà en base
    pub async fn restore_subscriptions(&self) -> CoreResult<()> {
        let pools = self.repository.find_all().await?;
        for pool in pools {
            self.listener.watch(pool.address).await;
        }
        Ok(())
    }
}
