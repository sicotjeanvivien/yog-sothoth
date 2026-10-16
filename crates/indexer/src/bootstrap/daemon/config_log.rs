//! The log lines this daemon writes about its own configuration, before acting on any of it:
//! what follows from a combination of variables, stated, then objected to when it deserves it.

use tracing::{info, warn};

use crate::bootstrap::{Config, IngestScope, TransactionArrival};

/// Name the acquisition couple that is running: the first line the process writes, and the only
/// place that names it.
pub(super) fn log_ingestion_mode(config: &Config) {
    info!(
        source = config.transaction_arrival.source().as_str(),
        scope = config.scope.as_str(),
        "ingestion mode"
    );
}

/// Object to the one couple that boots and cannot keep up: `logsSubscribe` on a program id, then a
/// fetch per transaction against a rate-limited tier. Nothing stops, most of what it sees is dropped.
pub(super) fn warn_saturating_couple(config: &Config) {
    if matches!(
        (&config.transaction_arrival, config.scope),
        (TransactionArrival::Fetched { .. }, IngestScope::Protocols)
    ) {
        warn!(
            "INGEST_SOURCE=rpc with INGEST_SCOPE=protocols subscribes to the whole program and fetches every transaction back, one request each. On a rate-limited endpoint most will be dropped and counted as fetch failures, with the process still up. INGEST_SOURCE=grpc is the mode this scope is for."
        );
    }
}

/// Print the probe's endpoint beside everything ingestion touches, `INGEST_TRANSACTION` included.
/// ⚠️ States the addresses only; whether they are independent is [`warn_probe_not_independent`]'s.
pub(super) fn log_probe_endpoints(config: &Config) {
    let ingestion = match config.transaction_arrival.fetched_from() {
        Some(fetch) => format!(
            "{} (stream) + {fetch} (getTransaction)",
            config.ingest_stream
        ),
        None => format!("{} (stream)", config.ingest_stream),
    };
    info!(
        probe = %config.network_status,
        %ingestion,
        "network status probe initialized — the chain reference the panel's slot and latency come from"
    );
}

/// Object when the probe reads an address ingestion already uses: one provider behind both
/// halves of the panel makes them fail together; see [`Config::probe_shares_ingestion_address`].
pub(super) fn warn_probe_not_independent(config: &Config) {
    if config.probe_shares_ingestion_address() {
        warn!(
            "NETWORK_STATUS_URL is the address ingestion already uses, so the dashboard's two \
             halves share one provider: the day it drops, the chain reading and the freshness \
             verdict go red together and neither says which failed. Point it elsewhere — the \
             probe costs one request every fifteen seconds. Compared as written; two spellings \
             of the same host would not be caught here."
        );
    }
}
