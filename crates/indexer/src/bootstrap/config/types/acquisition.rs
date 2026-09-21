use yog_bootstrap::Endpoint;

use super::IngestSource;

/// The acquisition model the daemon will run, **carrying what that model
/// needs** — and nothing the other one would have to ignore.
///
/// [`IngestSource`] says *which* model; this says which model **with its
/// ingredients**. The distinction is not cosmetic: `getTransaction` exists on
/// the notify-then-ask path and nowhere else, so its endpoint exists on this
/// variant and nowhere else. A `Config` holding an `Option<Endpoint>` beside a
/// source would have let `init_rpc_source` receive `None` — a state
/// `Config::load` cannot produce, unwrapped at runtime anyway, which is the
/// kind of guard a type removes rather than documents.
///
/// It is also what makes `INGEST_TRANSACTION_URL` stop being required under
/// `INGEST_SOURCE=grpc`: not a rule written somewhere and remembered, but the
/// absence of any code that reads it on that path.
pub(crate) enum Acquisition {
    /// Notify-then-ask: a `logsSubscribe` socket carries signatures, and
    /// `transaction` is where each one is fetched back from.
    Rpc { transaction: Endpoint },
    /// Delivered: the Yellowstone stream carries whole transactions, so there
    /// is no second call and nothing to configure for one.
    Grpc,
}

impl Acquisition {
    /// The endpoint this model fetches transactions back from, when it fetches
    /// at all.
    ///
    /// Its readers are the start-up line that prints everything ingestion
    /// touches — a probe is independent of *all* of it or of none, so a line
    /// naming only the stream would let an operator read an independence that
    /// the fetch endpoint denies — and nothing else. `init_rpc_source` receives
    /// the endpoint from the match arm instead, where it is not an `Option`.
    pub(crate) const fn transaction(&self) -> Option<&Endpoint> {
        match self {
            Self::Rpc { transaction } => Some(transaction),
            Self::Grpc => None,
        }
    }

    /// The axis this model is, stripped of what it carries.
    ///
    /// Its reader is the `ingestion mode` line the daemon writes at start-up,
    /// which names the running model with the very spelling the operator set —
    /// see [`IngestSource::as_str`].
    pub(crate) const fn source(&self) -> IngestSource {
        match self {
            Self::Rpc { .. } => IngestSource::Rpc,
            Self::Grpc => IngestSource::Grpc,
        }
    }
}
