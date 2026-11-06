//! Postgres implementation of `ApiKeyLookup`, and the operator's writes, which `yog-migrate`
//! alone calls under `yog_migrate`: the api reads, and may write nothing.

mod rows;

use async_trait::async_trait;
use rows::ActiveKeyRow;
pub use rows::ApiKeyListing;
use sqlx::PgPool;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{ActiveKey, ApiKey, ApiKeyLookup},
};

use crate::repositories::helper::map_sqlx_error;

#[derive(Clone)]
pub struct PgApiKeyRepository {
    pool: PgPool,
}

impl PgApiKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Store `key` for `client`, creating the client on its first key.
    /// ⚠️ A revoked client is refused: its key would be stored and never accepted.
    pub async fn create(&self, client: &str, key: &ApiKey) -> RepositoryResult<()> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        let owner = sqlx::query!(
            r#"
            INSERT INTO api_clients (name) VALUES ($1)
            ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
            RETURNING id, revoked_at
            "#,
            client,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;
        if owner.revoked_at.is_some() {
            return Err(RepositoryError::Conflict(format!(
                "client {client} is revoked"
            )));
        }

        sqlx::query!(
            "INSERT INTO api_keys (client_id, prefix, key_hash) VALUES ($1, $2, $3)",
            owner.id,
            key.prefix(),
            &key.hash()[..],
        )
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        tx.commit().await.map_err(map_sqlx_error)
    }

    /// Revoke the key named by `prefix`; `false` when no key under it was still active.
    pub async fn revoke(&self, prefix: &str) -> RepositoryResult<bool> {
        let revoked = sqlx::query!(
            "UPDATE api_keys SET revoked_at = now() WHERE prefix = $1 AND revoked_at IS NULL",
            prefix,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(revoked.rows_affected() == 1)
    }

    /// Every key, revoked or not, the newest first.
    pub async fn list(&self) -> RepositoryResult<Vec<ApiKeyListing>> {
        sqlx::query_as!(
            ApiKeyListing,
            r#"
            SELECT k.prefix, c.name AS client, k.created_at, k.revoked_at
              FROM api_keys k
              JOIN api_clients c ON c.id = k.client_id
             ORDER BY k.created_at DESC, k.id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}

#[async_trait]
impl ApiKeyLookup for PgApiKeyRepository {
    async fn active_keys(&self) -> RepositoryResult<Vec<ActiveKey>> {
        let rows = sqlx::query_as!(
            ActiveKeyRow,
            r#"
            SELECT k.key_hash, c.id AS client_id, c.name AS client_name
              FROM api_keys k
              JOIN api_clients c ON c.id = k.client_id
             WHERE k.revoked_at IS NULL
               AND c.revoked_at IS NULL
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ActiveKey::try_from).collect()
    }
}
