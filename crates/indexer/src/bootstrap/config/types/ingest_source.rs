use yog_bootstrap::EnvEnum;

/// Where the indexer's transactions come from.
///
/// Names the acquisition model rather than the wire protocol, because that
/// is what differs: `Rpc` **notifies then asks** — a `logsSubscribe` socket
/// carrying signatures, then one `getTransaction` per signature, which is
/// what caps throughput and drops `transaction_index`. `Grpc` **delivers** —
/// a single Yellowstone stream carrying whole transactions, no second call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IngestSource {
    Rpc,
    Grpc,
}

impl IngestSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Rpc => "rpc",
            Self::Grpc => "grpc",
        }
    }
}

impl EnvEnum for IngestSource {
    const EXPECTED: &'static str = "rpc or grpc";

    fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "rpc" => Some(Self::Rpc),
            "grpc" => Some(Self::Grpc),
            _ => None,
        }
    }
}
