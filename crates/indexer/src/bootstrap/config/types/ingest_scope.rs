use yog_bootstrap::EnvEnum;

/// What the listener subscribes to.
///
/// `Protocols` is one subscription per watched protocol, keyed on its
/// program id — full coverage, and the throughput the free tier cannot
/// sustain. `Pools` is one per row of `watched_pools`: that is where the
/// allowlist is enforced — **at the subscription, not by a filter** —
/// nothing downstream being aware of it.
///
/// Both outlive the RPC path. Pool scope is not legacy waiting to be
/// deleted: it stays meaningful on a gRPC stream, where the same addresses
/// go into the subscription filter instead of into one socket each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IngestScope {
    Protocols,
    Pools,
}

impl IngestScope {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Protocols => "protocols",
            Self::Pools => "pools",
        }
    }
}

impl EnvEnum for IngestScope {
    const EXPECTED: &'static str = "protocols or pools";

    fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "protocols" => Some(Self::Protocols),
            "pools" => Some(Self::Pools),
            _ => None,
        }
    }
}
