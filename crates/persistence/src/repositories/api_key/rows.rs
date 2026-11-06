use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{ActiveKey, ApiClient},
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

/// A key as an operator lists it: never the key, never its hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyListing {
    pub prefix: String,
    pub client: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
