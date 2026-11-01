//! One transaction through the real decoder, against fake repositories: what it writes and counts.

use std::future::Future;
use std::path::PathBuf;

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use yog_core::application::extraction::{
    InnerInstructionPayload, meteora::damm_v2::extractor::extract_wire_events,
};

use super::*;
use crate::application::services::MeteoraDammV2EventPersistor;
use crate::application::services::meteora::persistor_fakes::{
    Calls, build, build_with_failing_pool_upsert,
};
use crate::infra::from_rpc;

const PROTOCOL: Protocol = Protocol::MeteoraDammV2;
const UNKNOWN_HEX: &str = "abababababababab";

type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

pub(crate) fn processor(persistor: MeteoraDammV2EventPersistor) -> Arc<TransactionProcessor> {
    Arc::new(TransactionProcessor::new(
        Arc::new(ExtractionDispatcher::new()),
        Arc::new(EventPersistor::new(Arc::new(persistor))),
    ))
}

pub(crate) fn recording(calls: &Calls) -> Arc<TransactionProcessor> {
    processor(build(calls.clone()))
}

/// Two swaps on one pool.
pub(crate) fn double_swap() -> OnChainTransaction {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures/damm_v2/swap_double.json");
    let raw = std::fs::read_to_string(&path).expect("the fixture reads");
    let tx: EncodedConfirmedTransactionWithStatusMeta =
        serde_json::from_str(&raw).expect("the fixture parses");
    from_rpc(&tx).expect("the fixture adapts")
}

fn first_swap_payload(tx: &OnChainTransaction) -> Vec<u8> {
    let program = PROTOCOL.program_id();
    let index = extract_wire_events(tx, &program).events[0].event_index;
    tx.inner_instructions
        .iter()
        .filter(|ix| ix.program_id == program)
        .nth(usize::from(index))
        .expect("the first event's payload")
        .data
        .clone()
}

fn push_payload(tx: &mut OnChainTransaction, data: Vec<u8>) {
    tx.inner_instructions.push(InnerInstructionPayload {
        program_id: PROTOCOL.program_id(),
        data,
    });
}

/// A transaction holding only a known event whose content cannot be read.
pub(crate) fn unreadable_only() -> OnChainTransaction {
    let mut tx = double_swap();
    let unreadable = first_swap_payload(&tx)[..16].to_vec();
    tx.inner_instructions.clear();
    push_payload(&mut tx, unreadable);
    tx
}

fn snapshotted(body: impl Future<Output = ()>) -> Snapshot {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
            .block_on(body)
    });
    snapshotter.snapshot().into_vec()
}

fn counter(snapshot: &Snapshot, name: &str, labels: &[(&str, &str)]) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, value)| {
        let found = key.key().name() == name
            && labels.iter().all(|(label, wanted)| {
                key.key()
                    .labels()
                    .any(|l| l.key() == *label && l.value() == *wanted)
            });
        match value {
            DebugValue::Counter(n) if found => Some(*n),
            _ => None,
        }
    })
}

fn exited(snapshot: &Snapshot, outcome: &str) -> Option<u64> {
    counter(
        snapshot,
        "yog_indexer_index_transaction_exited_total",
        &[("outcome", outcome)],
    )
}

fn entered(snapshot: &Snapshot) -> Option<u64> {
    counter(snapshot, "yog_indexer_index_transaction_entered_total", &[])
}

fn failures_of(snapshot: &Snapshot, kind: &str) -> Option<u64> {
    counter(
        snapshot,
        "yog_indexer_extraction_failure_total",
        &[("kind", kind)],
    )
}

pub(crate) fn inserts(calls: &Calls) -> usize {
    calls
        .lock()
        .expect("the calls")
        .iter()
        .filter(|call| call.starts_with("insert:"))
        .count()
}

fn process(processor: Arc<TransactionProcessor>, tx: OnChainTransaction) -> Snapshot {
    snapshotted(async move {
        processor
            .process_transaction(PROTOCOL, &tx)
            .await
            .expect("no transaction-level failure");
    })
}

/// Mutation: the loop stopped at the first event, or `record_entered`, `record_exited` removed.
#[test]
fn a_transaction_writes_every_event_and_exits_ok() {
    let calls = Calls::default();
    let counted = process(recording(&calls), double_swap());

    assert_eq!(inserts(&calls), 2);
    assert_eq!(entered(&counted), Some(1));
    assert_eq!(exited(&counted, "ok"), Some(1));
}

/// Mutation: a failed pool write that aborts the transaction instead of being stepped over.
#[test]
fn a_failed_pool_write_does_not_stop_the_events_that_follow() {
    let calls = Calls::default();
    let counted = process(
        processor(build_with_failing_pool_upsert(calls.clone())),
        double_swap(),
    );

    assert_eq!(inserts(&calls), 2);
    assert_eq!(exited(&counted, "ok"), Some(1));
}

/// Mutation: `no_events` counted as `ok`, or `no_match` no longer counted.
#[test]
fn a_transaction_without_an_event_writes_nothing_and_says_so() {
    let calls = Calls::default();
    let mut tx = double_swap();
    tx.inner_instructions.clear();

    let counted = process(recording(&calls), tx);

    assert_eq!(inserts(&calls), 0);
    assert_eq!(exited(&counted, "no_events"), Some(1));
    assert_eq!(exited(&counted, "ok"), None);
    assert_eq!(
        counter(&counted, "yog_indexer_transactions_no_match_total", &[]),
        Some(1)
    );
}

/// Mutation: `report_diagnostics` reached only by a transaction without an event.
#[test]
fn an_unreadable_payload_is_counted_and_the_valid_events_are_written() {
    let calls = Calls::default();
    let mut tx = double_swap();
    let unreadable = first_swap_payload(&tx)[..16].to_vec();
    push_payload(&mut tx, unreadable);

    let counted = process(recording(&calls), tx);

    assert_eq!(inserts(&calls), 2);
    assert_eq!(exited(&counted, "ok"), Some(1));
    assert_eq!(failures_of(&counted, "borsh"), Some(1));
}

/// Mutation: `report_diagnostics` moved past the return of a transaction without an event, which
/// silences a format change: every payload of every transaction then fails.
#[test]
fn a_transaction_with_only_unreadable_payloads_still_counts_its_failure() {
    let calls = Calls::default();
    let counted = process(recording(&calls), unreadable_only());

    assert_eq!(inserts(&calls), 0);
    assert_eq!(exited(&counted, "no_events"), Some(1));
    assert_eq!(failures_of(&counted, "borsh"), Some(1));
}

/// Mutation: an unknown discriminator no longer counted, or under another code.
#[test]
fn an_unknown_discriminator_is_counted_with_its_code_and_the_valid_events_are_written() {
    let calls = Calls::default();
    let mut tx = double_swap();
    let unknown = [&first_swap_payload(&tx)[..8], &[0xAB; 8]].concat();
    push_payload(&mut tx, unknown);

    let counted = process(recording(&calls), tx);

    assert_eq!(inserts(&calls), 2);
    assert_eq!(exited(&counted, "ok"), Some(1));
    assert_eq!(
        counter(
            &counted,
            "yog_indexer_unknown_event_total",
            &[("discriminator", UNKNOWN_HEX)],
        ),
        Some(1)
    );
}

/// Mutation: two kinds swapped in `failure_kind`, or a failure no longer counted. One at a time: in
/// a single batch, two swapped kinds would still count one each.
/// ⚠️ Built by hand: no captured transaction reaches `Translation` or the overflow here.
#[test]
fn each_failure_is_counted_under_its_kind() {
    let cases = [
        (
            ExtractionFailure::AnchorDecode("truncated".to_string()),
            "anchor_decode",
        ),
        (
            ExtractionFailure::Borsh {
                event_name: "EvtSwap2",
                reason: "short".to_string(),
            },
            "borsh",
        ),
        (
            ExtractionFailure::Translation {
                event_name: "EvtSetPoolStatus",
                reason: "invalid".to_string(),
            },
            "translation",
        ),
        (
            ExtractionFailure::EventIndexOverflow { index: 65_536 },
            "event_index_overflow",
        ),
    ];

    for (failure, kind) in cases {
        let processor = recording(&Calls::default());
        let outcome = ExtractionOutcome {
            failures: vec![failure],
            ..ExtractionOutcome::default()
        };
        let counted = snapshotted(async {
            processor.report_diagnostics(&PROTOCOL, &Signature::default(), &outcome);
        });
        assert_eq!(failures_of(&counted, kind), Some(1), "{kind}");
    }
}
