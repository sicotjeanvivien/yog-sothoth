use yog_bootstrap::{ConfigError, SecretKey, required, required_secret_key};

/// The `ARCHIVE_STORE_*` variables, as read. What each one is, and what the
/// access key must be allowed to do, is said once, on the
/// [`StoreSettings`](crate::infra::StoreSettings) they become.
#[derive(Debug)]
pub(crate) struct StoreConfig {
    pub(crate) url: String,
    pub(crate) bucket: String,
    pub(crate) region: String,
    pub(crate) access_key: SecretKey,
    pub(crate) secret_key: SecretKey,
}

impl StoreConfig {
    /// Read every `ARCHIVE_STORE_*` variable. All are required.
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            url: required("ARCHIVE_STORE_URL")?,
            bucket: required("ARCHIVE_STORE_BUCKET")?,
            region: required("ARCHIVE_STORE_REGION")?,
            access_key: required_secret_key("ARCHIVE_STORE_ACCESS_KEY")?,
            secret_key: required_secret_key("ARCHIVE_STORE_SECRET_KEY")?,
        })
    }
}
