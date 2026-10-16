//! What the listener asks for, and how it reads what comes back. One `SubscribeRequest` covers
//! the whole subscription, so `SubscriptionWorker`'s per-address fleet has no counterpart here.
//! Each protocol's filter is named by `Protocol::as_str`: an update's protocol is a lookup of the
//! filters it matched, not a re-derivation from its account keys.

use std::collections::{HashMap, HashSet};

use solana_pubkey::Pubkey;
use yellowstone_grpc_proto::prelude::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterBlocksMeta,
    SubscribeRequestFilterTransactions,
};
use yog_core::domain::Protocol;

use crate::error::GrpcListenerError;

/// The name of the block-meta filter, the other half of the subscription; not a protocol name.
pub(crate) const BLOCK_META_FILTER: &str = "block_meta";

/// Build the subscription from what is watched; an empty set is refused rather than opening a
/// stream that stays silent for ever.
/// ⚠️ `vote: Some(false)` and `failed: Some(false)` are load-bearing: the adapter reads neither.
pub(crate) fn build_request(
    watched: &HashSet<(Protocol, Pubkey)>,
    from_slot: Option<u64>,
) -> Result<SubscribeRequest, GrpcListenerError> {
    let includes = account_includes(watched);

    if includes.is_empty() {
        return Err(GrpcListenerError::NoSubscriptionTargets);
    }

    let transactions = includes
        .into_iter()
        .map(|(protocol, account_include)| {
            (
                protocol.as_str().to_string(),
                SubscribeRequestFilterTransactions {
                    vote: Some(false),
                    failed: Some(false),
                    account_include,
                    ..Default::default()
                },
            )
        })
        .collect();

    Ok(SubscribeRequest {
        transactions,
        // ⚠️ Transactions carry no `block_time`: it comes from block-meta, paired by slot. Whole
        // blocks would carry it too, but bill every transaction of every block.
        blocks_meta: HashMap::from([(
            BLOCK_META_FILTER.to_string(),
            SubscribeRequestFilterBlocksMeta::default(),
        )]),
        // Same commitment as the WebSocket path, so switching source does not switch settledness.
        commitment: Some(CommitmentLevel::Confirmed as i32),
        from_slot,
        ..Default::default()
    })
}

/// One filter per protocol, listing that protocol's watched addresses.
/// ⚠️ Providers cap `account_include` per filter, some near ten: fine for program ids, the open
/// risk of a pool allowlist, where `cuckoo_account_include` is the compact form to reach for.
fn account_includes(watched: &HashSet<(Protocol, Pubkey)>) -> Vec<(Protocol, Vec<String>)> {
    let mut by_protocol: HashMap<Protocol, Vec<String>> = HashMap::new();
    for (protocol, address) in watched {
        by_protocol
            .entry(*protocol)
            .or_default()
            .push(address.to_string());
    }

    // Addresses are sorted so a test can assert on them; the filters land in a protobuf map,
    // where an order would be lost.
    let mut includes: Vec<_> = by_protocol.into_iter().collect();
    for (_, addresses) in includes.iter_mut() {
        addresses.sort();
    }
    includes
}

/// The protocol an update belongs to, from the filters it matched; `None` for a block-meta or any other name.
/// ⚠️ An update touching two protocols' addresses matches both filters, and the first one wins.
pub(crate) fn protocol_of(filters: &[String]) -> Option<Protocol> {
    filters.iter().find_map(|name| name.parse().ok())
}

#[cfg(test)]
#[path = "tests/subscription_tests.rs"]
mod tests;
