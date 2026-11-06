use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{ActiveKey, ApiClient, ApiKeyListing},
};

/// A key not revoked, of a client not revoked. A hash of another size than 32 bytes is an
/// integrity error.
pub(super) struct ActiveKeyRow {
    pub(super) key_hash: Vec<u8>,
    pub(super) client_id: i64,
    pub(super) client_name: String,
}

impl TryFrom<ActiveKeyRow> for ActiveKey {
    type Error = RepositoryError;

    fn try_from(row: ActiveKeyRow) -> Result<Self, Self::Error> {
        let hash = <[u8; 32]>::try_from(row.key_hash.as_slice()).map_err(|_| {
            RepositoryError::Integrity(format!("an api key hash of {} bytes", row.key_hash.len()))
        })?;

        Ok(ActiveKey {
            hash,
            client: ApiClient {
                id: row.client_id,
                name: row.client_name,
            },
        })
    }
}

/// A key as an operator lists it.
pub(super) struct ApiKeyListingRow {
    pub(super) prefix: String,
    pub(super) client: String,
    pub(super) created_at: DateTime<Utc>,
    pub(super) revoked_at: Option<DateTime<Utc>>,
}

impl From<ApiKeyListingRow> for ApiKeyListing {
    fn from(row: ApiKeyListingRow) -> Self {
        Self {
            prefix: row.prefix,
            client: row.client,
            created_at: row.created_at,
            revoked_at: row.revoked_at,
        }
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
