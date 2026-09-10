//! Tests for the subscription request and the routing back.
//!
//! ⚠️ **What these can and cannot say.** They pin what *we ask for*, exactly:
//! the request is our own object, so asserting on it is not the circular
//! exercise `transaction_adapter_tests` has to warn about. What no test here
//! can say is whether a provider **honours** the request — whether `vote:
//! false` really keeps votes off the wire, whether `from_slot` replays what we
//! think it replays. That is
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md`, and the
//! `yog_indexer_grpc_updates_total{kind}` counter is what it will read.

use super::*;

fn protocols(list: &[Protocol]) -> HashSet<Protocol> {
    list.iter().copied().collect()
}

fn pool(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn request(scope: IngestScope, pools: &[(Protocol, Pubkey)]) -> SubscribeRequest {
    build_request(
        scope,
        &protocols(&[Protocol::MeteoraDammV2]),
        &pools.iter().copied().collect(),
        None,
    )
    .expect("something is watched")
}

// ── the two flags the ticket says nothing will remind us of ─────────

/// ⚠️ **Votes and failed transactions are refused at the server**, and both
/// omissions are silent in a way this test exists to break.
///
/// Without `failed: false` the events of a reverted transaction are persisted
/// as though they happened — the adapter never looks at `meta.err`. Without
/// `vote: false` the bulk of Solana's stream is paid for, decoded and dropped,
/// and against a provider that marks votes `inner_instructions_none` each one
/// becomes a counted failure: an error metric that reads as a dead pipeline.
#[test]
fn the_transaction_filter_refuses_votes_and_failures() {
    let request = request(IngestScope::Protocols, &[]);

    let filter = request
        .transactions
        .get("meteora_damm_v2")
        .expect("one filter per watched protocol, named after it");

    assert_eq!(filter.vote, Some(false), "votes must not reach the wire");
    assert_eq!(
        filter.failed,
        Some(false),
        "a reverted transaction's events would be persisted as real ones"
    );
}

/// The block-meta subscription is not optional: it is the only source of
/// `block_time`, and without it every transaction waits for an instant that
/// never comes and leaves through the buffer's eviction counter.
#[test]
fn the_request_also_subscribes_to_block_metas() {
    let request = request(IngestScope::Protocols, &[]);

    assert!(
        request.blocks_meta.contains_key(BLOCK_META_FILTER),
        "no block-meta subscription means no timestamps at all"
    );
    assert_eq!(
        request.commitment,
        Some(CommitmentLevel::Confirmed as i32),
        "the same commitment the WebSocket path uses"
    );
}

// ── what each scope asks for ────────────────────────────────────────

/// `INGEST_SCOPE=protocols`: the program id, which is what makes the firehose
/// mode a single filter here where the RPC path needs a subscription per
/// address.
#[test]
fn the_protocol_scope_includes_the_program_id() {
    let request = request(IngestScope::Protocols, &[]);

    assert_eq!(
        request.transactions["meteora_damm_v2"].account_include,
        vec![Protocol::MeteoraDammV2.program_id().to_string()]
    );
}

/// `INGEST_SCOPE=pools`: the allowlist is enforced **at the subscription**, as
/// it is on the RPC path — not by a filter downstream.
#[test]
fn the_pool_scope_includes_the_watched_pools_grouped_by_protocol() {
    let request = request(
        IngestScope::Pools,
        &[
            (Protocol::MeteoraDammV2, pool(1)),
            (Protocol::MeteoraDammV2, pool(2)),
            (Protocol::MeteoraDlmm, pool(3)),
        ],
    );

    assert_eq!(
        request.transactions.len(),
        2,
        "one filter per protocol, not one per pool — the filter name is what \
         identifies the protocol on the way back, and a filter per pool would \
         multiply the account quota that is already the tight one"
    );
    // The expectation is sorted, not the result: `account_include` is a
    // repeated field whose order reaches the wire, and `pool_includes` sorts it
    // so this comparison is against a defined order rather than a `HashSet`
    // iteration.
    let mut expected = vec![pool(1).to_string(), pool(2).to_string()];
    expected.sort();
    assert_eq!(
        request.transactions["meteora_damm_v2"].account_include,
        expected
    );
    assert_eq!(
        request.transactions["meteora_dlmm"].account_include,
        vec![pool(3).to_string()]
    );
}

/// The two scopes are not interchangeable, and reading the wrong one is silent:
/// the subscription opens either way, and only what arrives differs.
#[test]
fn the_scope_decides_what_is_included_and_the_two_differ() {
    let watched = [(Protocol::MeteoraDammV2, pool(1))];

    let by_pools = request(IngestScope::Pools, &watched);
    let by_protocols = request(IngestScope::Protocols, &watched);

    assert_eq!(
        by_pools.transactions["meteora_damm_v2"].account_include,
        vec![pool(1).to_string()]
    );
    assert_eq!(
        by_protocols.transactions["meteora_damm_v2"].account_include,
        vec![Protocol::MeteoraDammV2.program_id().to_string()],
        "the protocol scope ignores the pool list entirely"
    );
}

/// ⚠️ Nothing watched is a refusal, not an empty subscription. A stream that
/// subscribes to nothing connects, succeeds, and goes quiet — a failure that
/// reads as a network fault and is a configuration one, and that returning an
/// empty request here would reproduce. The config-time `check_supported` that
/// used to catch the same shape earlier is gone since 10 September 2026; this
/// refusal is now the only one.
#[test]
fn nothing_watched_is_refused_rather_than_subscribed_empty() {
    let empty = build_request(
        IngestScope::Protocols,
        &HashSet::new(),
        &HashSet::new(),
        None,
    );

    assert!(matches!(
        empty,
        Err(GrpcListenerError::NoSubscriptionTargets)
    ));

    // And the same for the other scope, which reads a different collection —
    // one arm can be right while the other silently subscribes to nothing.
    let empty_pools = build_request(
        IngestScope::Pools,
        &protocols(&[Protocol::MeteoraDammV2]),
        &HashSet::new(),
        None,
    );

    assert!(
        matches!(empty_pools, Err(GrpcListenerError::NoSubscriptionTargets)),
        "the pool scope must not fall back on the protocols it was not asked for"
    );
}

/// `from_slot` is what a reconnection asks for, and its absence is what a first
/// connection asks for. Carried through untouched — the semantics belong to the
/// server, and the listener's fallback for a refused replay is at its own level.
#[test]
fn from_slot_is_carried_only_when_given() {
    assert_eq!(request(IngestScope::Protocols, &[]).from_slot, None);

    let resumed = build_request(
        IngestScope::Protocols,
        &protocols(&[Protocol::MeteoraDammV2]),
        &HashSet::new(),
        Some(1_234),
    )
    .expect("something is watched");

    assert_eq!(resumed.from_slot, Some(1_234));
}

// ── reading the protocol back ───────────────────────────────────────

/// The routing: a filter named after a protocol is how an update says which one
/// it belongs to, without re-deriving it from the account keys.
#[test]
fn an_update_names_its_protocol_through_the_filter_that_matched() {
    assert_eq!(
        protocol_of(&["meteora_damm_v2".to_string()]),
        Some(Protocol::MeteoraDammV2)
    );
    assert_eq!(
        protocol_of(&["meteora_dlmm".to_string()]),
        Some(Protocol::MeteoraDlmm)
    );
}

/// A block-meta matches its own filter and no protocol — and must not be
/// mistaken for one, since it carries no transaction to route.
#[test]
fn a_block_meta_filter_names_no_protocol() {
    assert_eq!(protocol_of(&[BLOCK_META_FILTER.to_string()]), None);
    assert_eq!(protocol_of(&[]), None);
    assert_eq!(protocol_of(&["something_else".to_string()]), None);
}

/// ⚠️ The two names must line up in **both** directions: the filter is built
/// from `Protocol::as_str` and read back through `FromStr`, and nothing but
/// this test connects the two. A protocol added with a name that does not
/// round-trip would subscribe correctly and route nothing, silently.
#[test]
fn every_protocol_name_round_trips_through_the_filter() {
    for protocol in Protocol::all() {
        assert_eq!(
            protocol_of(&[protocol.as_str().to_string()]),
            Some(*protocol),
            "{} does not survive the round trip",
            protocol.as_str()
        );
    }
}
