use yog_bootstrap::{ConfigError, Endpoint, required_endpoint};

use super::IngestSource;

/// How a transaction reaches this process — **and what that way of reaching it
/// needs configured**, which is the whole reason the type exists.
///
/// [`IngestSource`] says *which* way; this says which way **with its
/// ingredients**. The distinction is not cosmetic: a `getTransaction` exists
/// only where the arrival has to be asked for, so its endpoint is a field of
/// the variant that asks and of no other. A `Config` holding an
/// `Option<Endpoint>` beside a source would have let `init_rpc_source` receive
/// `None` — a state `Config::load` cannot produce, unwrapped at runtime anyway,
/// which is the kind of guard a type removes rather than documents.
///
/// It is also what makes `INGEST_TRANSACTION_URL` stop being required under
/// `INGEST_SOURCE=grpc`: not a rule written somewhere and remembered, but the
/// absence of any code that reads it on that path.
///
/// ⚠️ **The variants are named after what happens, not after what speaks.**
/// They were `Rpc` and `Grpc` for a day, which put a transport in a name whose
/// subject is an arrival — the same inversion this repository removed from its
/// variable names, where `SOLANA_RPC_HTTP` excluded nothing because a protocol
/// excludes nothing. The two literal spellings an operator writes stay where
/// they belong, on [`IngestSource`], which *is* the value of a variable.
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
    ///
    /// **The reading lives here rather than in `Config::load`** because the
    /// fact it encodes is about this type: `INGEST_TRANSACTION` exists for the
    /// variant that asks and for no other, so the variant is where that is
    /// written. It is the same arrangement [`IngestSource`] and `IngestScope`
    /// already have through `EnvEnum` — a setting knows how to read itself —
    /// and it keeps `Config::load` a list of one-liners, where a six-line match
    /// stopped the struct literal being scannable.
    pub(crate) fn from_source(source: IngestSource) -> Result<Self, ConfigError> {
        Ok(match source {
            IngestSource::Rpc => Self::Fetched {
                from: required_endpoint("INGEST_TRANSACTION")?,
            },
            // Nothing to read: the stream delivers the transaction whole.
            IngestSource::Grpc => Self::Delivered,
        })
    }

    /// Where the transaction is fetched back from, when it has to be fetched
    /// at all.
    ///
    /// Its reader is the start-up line that prints everything ingestion
    /// touches — a probe is independent of *all* of it or of none, so a line
    /// naming only the stream would let an operator read an independence that
    /// this endpoint denies — and nothing else. `init_rpc_source` receives the
    /// endpoint from the match arm instead, where it is not an `Option`.
    pub(crate) const fn fetched_from(&self) -> Option<&Endpoint> {
        match self {
            Self::Fetched { from } => Some(from),
            Self::Delivered => None,
        }
    }

    /// The setting this arrival came from, stripped of what it carries.
    ///
    /// Its reader is the `ingestion mode` line the daemon writes at start-up,
    /// which names the running model with the very spelling the operator set —
    /// see [`IngestSource::as_str`].
    pub(crate) const fn source(&self) -> IngestSource {
        match self {
            Self::Fetched { .. } => IngestSource::Rpc,
            Self::Delivered => IngestSource::Grpc,
        }
    }
}
