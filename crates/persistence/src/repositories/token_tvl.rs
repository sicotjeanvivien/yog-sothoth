//! PostgreSQL implementation of [`TokenTvlLookup`]: yog-analytic's TVL by token, each token's
//! identity and the protocol's reading joined in the same read.

mod cursor;
mod page;
mod query;
mod rows;

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{Protocol, TokenTvlCursor, TokenTvlLookup, TokenTvlOrder, TokenTvlPage},
};

pub struct PgTokenTvlRepository {
    pool: PgPool,
}

impl PgTokenTvlRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenTvlLookup for PgTokenTvlRepository {
    async fn list(
        &self,
        protocol: Protocol,
        order: TokenTvlOrder,
        cursor: Option<TokenTvlCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<TokenTvlPage>> {
        let keys = cursor::keys_of(cursor.as_ref())?;
        let rows = match order {
            TokenTvlOrder::Tvl => query::by_tvl(&self.pool, protocol, &keys, limit).await?,
            TokenTvlOrder::Locked => query::by_locked(&self.pool, protocol, &keys, limit).await?,
        };
        page::page_of(rows, protocol, order, keys.given, limit)
    }
}
