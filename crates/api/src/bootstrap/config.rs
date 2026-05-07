use std::net::SocketAddr;
use yog_bootstrap::{ConfigError, SecretUrl, required};

#[derive(Clone)]
pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    pub(crate) bind_addr: SocketAddr,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let bind_addr_raw = required("API_BIND_ADDR")?;
        let bind_addr =
            bind_addr_raw
                .parse::<SocketAddr>()
                .map_err(|_| ConfigError::InvalidValue {
                    key: "API_BIND_ADDR".to_string(),
                    value: bind_addr_raw,
                    expected: "a host:port socket address (e.g. 127.0.0.1:3000)",
                })?;

        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL_API")?),
            bind_addr,
        })
    }
}
