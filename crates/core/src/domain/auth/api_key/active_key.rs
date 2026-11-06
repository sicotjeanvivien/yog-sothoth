use crate::domain::ApiClient;

/// A key the API accepts: its hash, and the client it opens the API to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveKey {
    pub hash: [u8; 32],
    pub client: ApiClient,
}
