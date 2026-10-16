use yog_bootstrap::{ConfigError, Endpoint, required_endpoint};

use super::IngestSource;

/// How a transaction reaches this process, with what that way needs configured: the
/// `getTransaction` endpoint is a field of the variant that asks, and of no other.
/// ⚠️ Named after what happens, not what speaks; the spellings an operator writes are [`IngestSource`]'s.
pub(crate) enum TransactionArrival {
    /// Asked for: the stream carries a signature, and `from` is where the
    /// transaction itself is fetched back.
    Fetched { from: Endpoint },
    /// Handed over: the Yellowstone stream carries the whole transaction, so
    /// there is no second call and nothing to configure for one.
    Delivered,
}

impl TransactionArrival {
    /// Build the arrival the setting selects, reading what that arrival needs.
    pub(crate) fn from_source(source: IngestSource) -> Result<Self, ConfigError> {
        Ok(match source {
            IngestSource::Rpc => Self::Fetched {
                from: required_endpoint("INGEST_TRANSACTION")?,
            },
            // Nothing to read: the stream delivers the transaction whole.
            IngestSource::Grpc => Self::Delivered,
        })
    }

    /// Where the transaction is fetched back from, when it is; read by the start-up line only,
    /// `init_rpc_source` gets the endpoint from the match arm.
    pub(crate) const fn fetched_from(&self) -> Option<&Endpoint> {
        match self {
            Self::Fetched { from } => Some(from),
            Self::Delivered => None,
        }
    }

    /// The setting this arrival came from, for the start-up `ingestion mode` line.
    pub(crate) const fn source(&self) -> IngestSource {
        match self {
            Self::Fetched { .. } => IngestSource::Rpc,
            Self::Delivered => IngestSource::Grpc,
        }
    }
}
