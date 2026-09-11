//! What the listener asks for, and how it reads what comes back.
//!
//! One `SubscribeRequest` describes the whole subscription — where the JSON-RPC
//! path needs one WebSocket per watched address, because `logsSubscribe` takes
//! exactly one pubkey per `mentions` filter. That difference is why
//! `SubscriptionWorker` has no counterpart on this path: a fleet exists to work
//! around a limit this protocol does not have.
//!
//! Kept apart from `listener` and free of I/O so that the request — the one
//! thing here that can be wrong in a way no local test would otherwise catch —
//! is built by a pure function with its own tests.
//!
//! # The filter names are the routing
//!
//! A `SubscribeUpdate` carries `filters`: the names of the request's filters it
//! matched. So each protocol gets a filter **named after it**, and reading a
//! transaction's protocol back is a lookup, not a re-derivation from the
//! account keys. `Protocol::as_str` is that name, which also makes it the same
//! string as the one in every log line, metric label and SQL row.

use std::collections::{HashMap, HashSet};

use solana_pubkey::Pubkey;
use yellowstone_grpc_proto::prelude::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterBlocksMeta,
    SubscribeRequestFilterTransactions,
};
use yog_core::domain::Protocol;

use crate::error::GrpcListenerError;

/// The name of the block-meta filter — the other half of the subscription.
///
/// Not a protocol, so it can never collide with one: `Protocol::as_str`
/// produces `meteora_*` names, and a future protocol called `block_meta` would
/// be a naming problem long before it was one here.
pub(crate) const BLOCK_META_FILTER: &str = "block_meta";

/// Build the subscription from what is watched.
///
/// # ⚠️ The two filter flags that are not decoration
///
/// **`vote: Some(false)`** — votes are the bulk of Solana's transaction stream.
/// Without this the subscription pays for them (billing is by bandwidth here,
/// not by request), decodes each one, and throws it away; and a provider that
/// reports them with `inner_instructions_none` would turn every single one into
/// a counted skip-and-log failure, so the pipeline's error metric would read as
/// a total outage while nothing was wrong.
///
/// **`failed: Some(false)`** — the adapter does not look at `meta.err`, so
/// events from a reverted transaction would be persisted as though they had
/// happened. The JSON-RPC path drops them client-side in
/// `infra/rpc/dispatcher/filters/failed_transaction.rs`; here the filter moves
/// to the server, which is one of the stated gains of the change.
///
/// ⚠️ **What has no counterpart is `InvocationFilter`.** gRPC's
/// `account_include` matches on account *keys*, so a transaction that merely
/// references the program through an address lookup table still arrives. It
/// costs a translation and an extraction that find nothing to decode — waste,
/// not error — and the same filter cannot be expressed server-side. Left as
/// noise on purpose rather than reproduced client-side for a case that
/// `damm_v2` measured at a handful per thousand.
///
/// # Errors
///
/// [`GrpcListenerError::NoSubscriptionTargets`] when nothing is watched. That
/// is a configuration failure that *names itself* — the alternative is a stream
/// that opens, subscribes to nothing, and stays silent for ever, which reads as
/// a network fault and is not one. A `check_supported` in `bootstrap/config`
/// used to refuse that couple before it got here; it was deleted on
/// 10 September 2026 once the daemon started registering what it watches, and
/// this refusal is what remains — the last line of defence rather than the
/// first.
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
        // The other half of the pairing: `SubscribeUpdateTransaction` carries no
        // `block_time`, and `TransactionPosition::timestamp` may not be
        // optional. Subscribing to whole blocks would carry the time in the
        // same message and was rejected on cost — paying, by the byte, for
        // every transaction of every block to keep a handful.
        blocks_meta: HashMap::from([(
            BLOCK_META_FILTER.to_string(),
            SubscribeRequestFilterBlocksMeta::default(),
        )]),
        // The same commitment the WebSocket path subscribes at
        // (`SubscriptionWorker`), so switching source does not quietly switch
        // how settled the data is.
        commitment: Some(CommitmentLevel::Confirmed as i32),
        from_slot,
        ..Default::default()
    })
}

/// One filter per protocol, listing that protocol's watched addresses — its
/// program id, or its pools.
///
/// Grouped by protocol rather than one filter per address, for two reasons that
/// point the same way: the filter name is what identifies the protocol on the
/// way back, and a filter per address would multiply a quota that is already
/// the tight one. `account_include` matches any of its entries, so a program id
/// and pools of the same protocol could share a filter without changing what
/// arrives — the program id already covers them.
///
/// ⚠️ **That quota is the open risk of the pool allowlist.** Providers cap
/// `account_include` per filter, and the surveyed ceilings vary by two orders of
/// magnitude — ample at some, of the order of ten at others. Indifferent while
/// the list is a program id or two; structural the day the allowlist is what
/// goes in. The count is logged at subscription time so it is visible before a
/// provider refuses it, and `cuckoo_account_include` — the compact form, already
/// available in this crate — is what the answer would be built on.
fn account_includes(watched: &HashSet<(Protocol, Pubkey)>) -> Vec<(Protocol, Vec<String>)> {
    let mut by_protocol: HashMap<Protocol, Vec<String>> = HashMap::new();
    for (protocol, address) in watched {
        by_protocol
            .entry(*protocol)
            .or_default()
            .push(address.to_string());
    }

    // ⚠️ The addresses inside a filter are sorted; the filters themselves are not.
    // The difference is not taste: `account_include` is a repeated field, so its
    // order survives into the request and a test can assert on it, while the
    // filters end up in a protobuf **map** — unordered by definition, and
    // collected into a `HashMap` here. Sorting them would be work whose result
    // is discarded one line later, and a comment claiming it made the request
    // reproducible would be false.
    let mut includes: Vec<_> = by_protocol.into_iter().collect();
    for (_, addresses) in includes.iter_mut() {
        addresses.sort();
    }
    includes
}

/// The protocol an update belongs to, read from the filters it matched.
///
/// `None` for an update that matched no protocol filter — the block-meta
/// updates, and anything a provider sends that was not asked for. The caller
/// decides what that means; here it is simply not a protocol.
///
/// ⚠️ An update can match **several** filters, and the first match wins. One
/// filter per protocol makes the *names* distinct, not the transactions: one
/// that touches watched addresses of two protocols matches both. Today that
/// cannot happen only because a single protocol has a working extractor, so
/// only its addresses are watched. The day a second one is written, taking the
/// first is still the only defensible answer — the transaction is genuinely
/// both, and the pipeline handles one protocol at a time.
pub(crate) fn protocol_of(filters: &[String]) -> Option<Protocol> {
    filters.iter().find_map(|name| name.parse().ok())
}

#[cfg(test)]
#[path = "subscription_tests.rs"]
mod tests;
