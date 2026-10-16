use yog_bootstrap::EnvEnum;

/// What the daemon registers with the source: a program id per protocol, or one address per
/// `watched_pools` row. The allowlist is enforced at the subscription, not by a filter.
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
