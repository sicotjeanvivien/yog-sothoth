use yog_bootstrap::{ConfigError, SecretUrl, required};

pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    pub(crate) bind_addr: String,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL_API")?),
            bind_addr: required("API_BIND_ADDR")?,
        })
    }
}
