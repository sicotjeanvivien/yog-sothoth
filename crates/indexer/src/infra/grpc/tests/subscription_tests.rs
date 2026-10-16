//! Tests for the subscription request and the routing back: what we ask for, exactly. Whether a
//! provider honours it takes a real one, and `yog_indexer_grpc_updates_total{kind}` will say.

use super::*;

fn pool(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn damm_v2_program() -> (Protocol, Pubkey) {
    (
        Protocol::MeteoraDammV2,
        Protocol::MeteoraDammV2.program_id(),
    )
}

fn request(watched: &[(Protocol, Pubkey)]) -> SubscribeRequest {
    build_request(&watched.iter().copied().collect(), None).expect("something is watched")
}

// ── the two flags nothing else will remind us of ────────────────────

/// ⚠️ Both omissions are silent: without `failed: false` reverted events are persisted, without
/// `vote: false` the stream's bulk is paid for and may count as failures.
#[test]
fn the_transaction_filter_refuses_votes_and_failures() {
    let request = request(&[damm_v2_program()]);

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

/// The block-meta subscription is the only source of `block_time`: without it every transaction
/// waits for ever and leaves through the eviction counter.
#[test]
fn the_request_also_subscribes_to_block_metas() {
    let request = request(&[damm_v2_program()]);

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

// ── what is asked for ───────────────────────────────────────────

/// A watched protocol is its program id: one filter, where the RPC path needs one per address.
#[test]
fn a_watched_program_id_is_included_as_is() {
    let request = request(&[damm_v2_program()]);

    assert_eq!(
        request.transactions["meteora_damm_v2"].account_include,
        vec![Protocol::MeteoraDammV2.program_id().to_string()]
    );
}

/// Watched pools: the allowlist is enforced at the subscription, not by a downstream filter.
#[test]
fn watched_pools_are_grouped_by_protocol() {
    let request = request(&[
        (Protocol::MeteoraDammV2, pool(1)),
        (Protocol::MeteoraDammV2, pool(2)),
        (Protocol::MeteoraDlmm, pool(3)),
    ]);

    assert_eq!(
        request.transactions.len(),
        2,
        "one filter per protocol, not one per pool — the filter name is what \
         identifies the protocol on the way back, and a filter per pool would \
         multiply the account quota that is already the tight one"
    );
    // The expectation is sorted, not the result: `account_includes` sorts the repeated field.
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

/// ⚠️ Nothing watched is a refusal: an empty subscription connects and goes quiet, reading as a
/// network fault.
#[test]
fn nothing_watched_is_refused_rather_than_subscribed_empty() {
    assert!(matches!(
        build_request(&HashSet::new(), None),
        Err(GrpcListenerError::NoSubscriptionTargets)
    ));
}

/// `from_slot` is carried through untouched; its semantics are the server's.
#[test]
fn from_slot_is_carried_only_when_given() {
    assert_eq!(request(&[damm_v2_program()]).from_slot, None);

    let resumed = build_request(&HashSet::from([damm_v2_program()]), Some(1_234))
        .expect("something is watched");

    assert_eq!(resumed.from_slot, Some(1_234));
}

// ── reading the protocol back ───────────────────────────────────────

/// The routing: a filter named after a protocol says which one an update belongs to.
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

/// A block-meta matches its own filter and no protocol.
#[test]
fn a_block_meta_filter_names_no_protocol() {
    assert_eq!(protocol_of(&[BLOCK_META_FILTER.to_string()]), None);
    assert_eq!(protocol_of(&[]), None);
    assert_eq!(protocol_of(&["something_else".to_string()]), None);
}

/// ⚠️ `Protocol::as_str` and `FromStr` must round-trip: a name that does not would subscribe
/// correctly and route nothing.
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
