//! PostgreSQL implementation of [`VestingUnlockLookup`]: yog-analytic's vesting releases of its
//! last reading, each pool's token symbols joined in the same read.

mod cursor;
mod page;
mod query;
mod rows;

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        Protocol, VestingUnlockCursor, VestingUnlockLookup, VestingUnlockOrder, VestingUnlockPage,
        Window,
    },
};

use crate::repositories::helper::interval_of;

pub struct PgVestingUnlockRepository {
    pool: PgPool,
}

impl PgVestingUnlockRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VestingUnlockLookup for PgVestingUnlockRepository {
    async fn list(
        &self,
        protocol: Protocol,
        window: Window,
        order: VestingUnlockOrder,
        cursor: Option<VestingUnlockCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<VestingUnlockPage>> {
        let keys = cursor::keys_of(cursor.as_ref())?;
        let span = interval_of(window.duration());
        let rows = match order {
            VestingUnlockOrder::Time => {
                query::by_time(&self.pool, protocol, span, &keys, limit).await?
            }
            VestingUnlockOrder::Amount => {
                query::by_amount(&self.pool, protocol, span, &keys, limit).await?
            }
        };
        page::page_of(rows, protocol, window, order, keys.given, limit)
    }
}
