use yog_bootstrap::EnvEnum;

/// Where the indexer's transactions come from, named by acquisition model: `Rpc` notifies then
/// asks (one `getTransaction` per signature, no `transaction_index`), `Grpc` delivers them whole.
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
