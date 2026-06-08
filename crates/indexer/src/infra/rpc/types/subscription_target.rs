use solana_pubkey::Pubkey;
use yog_core::domain::Protocol;

/// A single RPC subscription target.
///
/// Solana's `logsSubscribe` `mentions` filter accepts exactly one pubkey per
/// subscription. We spawn one worker per target; each target carries the
/// protocol context so downstream stages can tag events appropriately.
///
/// In Phase 1 the target is a pool address (bounded allowlist to work around
/// Helius free-tier limits — 5 concurrent WS connections, 10 req/s). Once the
/// RPC path is upgraded, targets will revert to program IDs for full
/// protocol-centric coverage.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SubscriptionTarget {
    pub protocol: Protocol,
    pub mention: Pubkey,
}

impl SubscriptionTarget {
    pub(crate) fn new(protocol: Protocol, mention: Pubkey) -> Self {
        Self { protocol, mention }
    }
}
