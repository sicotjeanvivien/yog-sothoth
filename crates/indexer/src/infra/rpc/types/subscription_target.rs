use solana_pubkey::Pubkey;
use yog_core::domain::Protocol;

/// One `logsSubscribe` target, a program id or a pool, with its protocol: the `mentions` filter
/// takes exactly one pubkey, so there is one worker per target.
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
