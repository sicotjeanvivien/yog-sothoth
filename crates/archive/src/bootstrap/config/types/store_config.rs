//! How the bucket's settings are read from the environment. The settings
//! themselves — what each field is — are the infrastructure's
//! ([`StoreSettings`]); reading variables is the configuration's job, so it
//! stays here.

use yog_bootstrap::{ConfigError, required, required_secret_key};

use crate::infra::StoreSettings;

impl StoreSettings {
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
