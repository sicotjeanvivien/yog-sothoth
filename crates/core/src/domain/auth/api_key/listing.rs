use chrono::{DateTime, Utc};

/// A key as an operator lists it: never the key, never its hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyListing {
    pub prefix: String,
    pub client: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}
