use std::sync::Arc;

use chrono::Utc;
use solana_pubkey::pubkey;
use tracing::info;
use yog_core::{
    domain::{Protocol, WatchedPool, WatchedPoolRepository},
    CoreResult,
};

use crate::infra::RpcListener;

/// Manages the lifecycle of pool subscriptions.
///
/// Single responsibility : keep the database and the WebSocket listener
/// in sync — a pool persisted in the database must always have an active
/// WebSocket subscription, and vice versa.
pub struct WatchedPoolService {
    listener: Arc<RpcListener>,
    repository: Arc<dyn WatchedPoolRepository>,
}

impl WatchedPoolService {
    pub(crate) fn new(
        listener: Arc<RpcListener>,
        repository: Arc<dyn WatchedPoolRepository>,
    ) -> Self {
        Self {
            listener,
            repository,
        }
    }

    /// Persist a pool and register its WebSocket subscription.
    pub async fn _watch(&self, pool: WatchedPool) -> CoreResult<()> {
        self.repository.add(&pool).await?;
        self.listener.watch(pool.pool_address.to_string()).await;
        info!(address = %pool.pool_address, protocol = %pool.protocol, "pool watch registered");
        Ok(())
    }

    /// Remove a pool from persistence and cancel its WebSocket subscription.
    pub async fn _unwatch(&self, address: &str) -> CoreResult<()> {
        self.repository.remove(address).await?;
        self.listener.unwatch(address.to_string()).await;
        info!(address = %address, "pool watch removed");
        Ok(())
    }

    /// On daemon startup, resubscribe to all pools persisted in the database.
    /// Ensures no subscription is lost across restarts.
    pub async fn restore_subscriptions(&self) -> CoreResult<()> {
        self.watched_pool_test().await?;
        let pools = self.repository.find_all().await?;
        let count = pools.len();
        for pool in pools {
            self.listener.watch(pool.pool_address.to_string()).await;
        }
        info!(count, "subscriptions restored from database");
        Ok(())
    }

    async fn watched_pool_test(&self) -> CoreResult<()> {
        let address = "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j";
        if !self.repository.exists(address).await? {
            self.repository
                .add(&WatchedPool {
                    pool_address: pubkey!("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j"),
                    protocol: Protocol::DammV2,
                    token_a_mint: pubkey!("E3r3rs6C9bZbokaPiMEwmvPUtcd6CE2nuK8RSMQdE64E"),
                    token_b_mint: pubkey!("HK2HggD4Eg1tAyr3gnRvNG32Z8v7s1NQGjH77b14qvsx"),
                    token_a_decimals: 6,
                    token_b_decimals: 6,
                    added_at: Utc::now(),
                })
                .await?;
        }
        Ok(())
    }
}
