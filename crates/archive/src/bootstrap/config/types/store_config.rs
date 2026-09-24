use yog_bootstrap::{ConfigError, SecretKey, required, required_secret_key};

/// An S3-compatible bucket and the credentials that may write to it.
#[derive(Debug)]
pub(crate) struct StoreConfig {
    /// The endpoint, e.g. `https://s3.fr-par.scw.cloud`. Plain `http://` is
    /// accepted for a local MinIO.
    pub(crate) url: String,
    pub(crate) bucket: String,
    pub(crate) region: String,
    /// Access key id and secret. The key is meant to be **write-only**: a
    /// compromised server must not be able to delete the backups.
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
