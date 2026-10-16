//! Daemon configuration, loaded from the environment. `INGEST_SOURCE` (the acquisition model)
//! and `INGEST_SCOPE` (protocols or pools) are orthogonal; each variable names its function.
//! ⚠️ `INGEST_TRANSACTION` is read inside [`TransactionArrival::Fetched`] only, so not under grpc.
//! ⚠️ `NETWORK_STATUS` is outside the `INGEST_*` family on purpose: a probe riding the ingestion's
//! link would go red with it, and the panel could no longer say which side failed.

use yog_bootstrap::{
    ConfigError, Endpoint, SecretUrl, optional_secret_url, parse_required_enum, parse_required_u32,
    required_endpoint, required_endpoint_allowing_header, required_secret_url,
};

mod types;

pub(crate) use types::{IngestScope, IngestSource, TransactionArrival};

pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    /// Where the notifications the ingestion listens to come from.
    pub(crate) ingest_stream: Endpoint,
    /// How a transaction reaches this process, with what that way needs (the `getTransaction`
    /// endpoint on the arm that asks).
    pub(crate) transaction_arrival: TransactionArrival,
    /// The external chain reference the health probe reads; not an ingestion endpoint.
    pub(crate) network_status: Endpoint,
    pub(crate) worker_max_retries: u32,
    pub(crate) scope: IngestScope,
    /// The Healthchecks.io check of the ingestion alarm; optional, production's compose requires it.
    pub(crate) heartbeat_url: Option<SecretUrl>,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let source = parse_required_enum::<IngestSource>("INGEST_SOURCE")?;
        let scope = parse_required_enum::<IngestScope>("INGEST_SCOPE")?;

        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_INDEXER")?,
            // ⚠️ The wide door promises the holder sends the header it carries; both listeners
            // do, through `infra::endpoint::credential`.
            ingest_stream: required_endpoint_allowing_header("INGEST_STREAM")?,
            transaction_arrival: TransactionArrival::from_source(source)?,
            network_status: required_endpoint("NETWORK_STATUS")?,
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            scope,
            heartbeat_url: optional_secret_url("INDEXER_HEARTBEAT_URL"),
        })
    }

    /// Whether the health probe reads an address ingestion already uses: one address written
    /// twice, compared on [`Endpoint`]'s `Display`; two spellings of one host compare different.
    /// ⚠️ Not on the assembled URL: every `.expose()` site is counted by a guard.
    pub(crate) fn probe_shares_ingestion_address(&self) -> bool {
        let probe = self.network_status.to_string();
        self.ingest_stream.to_string() == probe
            || self
                .transaction_arrival
                .fetched_from()
                .is_some_and(|fetch| fetch.to_string() == probe)
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
