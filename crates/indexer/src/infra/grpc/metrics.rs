//! Counters for the gRPC ingestion path — two families, one per owner: the
//! slot/time buffer, and the listener that drives it.
//!
//! Same shape as `infra/rpc/dispatcher/metrics.rs`: names declared as
//! constants, descriptions registered once at startup, increments behind named
//! methods so a call site never spells a metric name.

use metrics::{counter, describe_counter};

/// Payloads dropped because their slot never received a block time.
///
/// ⚠️ **This is the number the next slice goes looking for.** The buffer's
/// bounds are ceilings picked without a measurement — see
/// `slot_timestamp_buffer`'s module docs — so this counter staying at zero is
/// what says the guess was generous, and any movement is what says it was not.
/// `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` replaces the guess by
/// reading it.
///
/// ⚠️ **The unit is a transaction, not an `InnerInstructionPayload`** — hence
/// the name. This crate already uses "payload" for what `transaction_adapter`
/// produces, and the README states that one mainnet transaction yields 2 of
/// those through one adapter and 14 through the other. The buffer holds whole
/// transactions, so a reading of 100 here is 100 transactions and some larger
/// number of adapter payloads. Named for the unit after review pointed out that
/// the measuring ticket had no way to tell which one it was holding.
const TRANSACTIONS_EVICTED: &str = "yog_indexer_grpc_untimestamped_transactions_total";

/// Which bound forced the eviction, on every increment of [`TRANSACTIONS_EVICTED`].
///
/// ⚠️ **Not decoration — without it the counter misleads the ticket that reads
/// it.** Two very different evictions share this metric: a slot that waited
/// past `MAX_PENDING_SLOTS`, and a burst that hit `MAX_PENDING_PAYLOADS` while
/// the slot may have been one message from resolving. The two ceilings cross at
/// **32 payloads per slot** (8 192 / 256), so above that rate the payload bound
/// is the binding one: at ~100 payloads per slot it fires at ~81 slots, not the
/// 256 the docs advertise as the wait window. An unlabelled counter would be
/// read as "the slot ceiling was too small", and raising it would change
/// nothing. Found in review, 8 September 2026.
#[derive(Debug, Clone, Copy)]
pub(crate) enum EvictionReason {
    /// Too many slots waiting: this one is the oldest.
    SlotBound,
    /// Too many payloads held across all pending slots.
    PayloadBound,
    /// The slot's block-meta arrived and carried **no** `block_time`, or the
    /// slot was abandoned by a fork: nothing will ever resolve it.
    ///
    /// ⚠️ **Distinct from [`Self::SlotBound`] for the same reason that one is
    /// distinct from [`Self::PayloadBound`]**, and it is the more misleading
    /// confusion of the two. Without this label such a slot waits out the whole
    /// window and then leaves counted `slot_bound` — so
    /// `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` would read "the wait
    /// window is too small" and raise `MAX_PENDING_SLOTS`, which changes
    /// nothing at all: the meta already came, empty.
    Unresolvable,
}

impl EvictionReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SlotBound => "slot_bound",
            Self::PayloadBound => "payload_bound",
            Self::Unresolvable => "unresolvable",
        }
    }
}

pub struct GrpcBufferMetrics;

impl GrpcBufferMetrics {
    /// Call once at startup to register the descriptions with the Prometheus
    /// exporter — `bootstrap/daemon.rs`, beside the crate's other families.
    ///
    /// Without that call the counter still works and exports with no HELP
    /// text, which is unreadable to anyone without the source — that is,
    /// exactly its reader.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TRANSACTIONS_EVICTED,
            "Transactions dropped because their slot never received a block time, \
             labelled by the bound that forced it"
        );
    }

    pub(crate) fn record_evicted(count: usize, reason: EvictionReason) {
        counter!(TRANSACTIONS_EVICTED, "reason" => reason.as_str()).increment(count as u64);
    }
}

// ── The listener's own family ───────────────────────────────────────────────

/// Updates taken off the stream, by what they were.
///
/// ⚠️ **This one is read to check a subscription filter, not a pipeline.** The
/// request sets `vote: Some(false)` and `failed: Some(false)`, and a provider
/// that ignores either would show up here as a `transaction` rate an order of
/// magnitude above the pool activity — the "counted skip-and-log failures that
/// read as a broken pipeline" the adapter's module docs warn about. Nothing
/// else in the process can tell a filter that was honoured from one that was
/// not.
const UPDATES_RECEIVED: &str = "yog_indexer_grpc_updates_total";

/// Transactions that failed to translate into an `OnChainTransaction`.
///
/// Skip-and-log, per transaction: the stream keeps running. The label is the
/// error's kind, and `from_grpc`'s doc-comment lists what can appear.
const ADAPTER_FAILURES: &str = "yog_indexer_grpc_adapter_failures_total";

/// Transactions handed downstream, timestamped.
const TRANSACTIONS_EMITTED: &str = "yog_indexer_grpc_transactions_emitted_total";

/// Times the downstream channel was full when a transaction was ready.
///
/// ⚠️ **This is a back-pressure counter, not a loss counter** — and that is the
/// difference with `infra/rpc/dispatcher`'s `downstream_saturated`, which
/// counts *drops*. On the RPC path a dropped signature can be asked for again;
/// here the transaction arrived once, over a stream billed by bandwidth, and
/// asking again means a `getTransaction` — the very call this path exists to
/// remove. So the listener waits instead of dropping, and what this counts is
/// the wait: the consumer is not keeping up, and the stream is being slowed to
/// its speed.
const DOWNSTREAM_FULL: &str = "yog_indexer_grpc_downstream_full_total";

/// What an update was, for [`UPDATES_RECEIVED`].
#[derive(Debug, Clone, Copy)]
pub(crate) enum UpdateKind {
    Transaction,
    BlockMeta,
    /// A server keep-alive. Answered, not merely counted — see the listener.
    Ping,
    /// The answer to one of ours.
    Pong,
    /// Anything the subscription did not ask for. Non-zero here means the
    /// request and the reader disagree about what was subscribed to.
    Other,
}

impl UpdateKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Transaction => "transaction",
            Self::BlockMeta => "block_meta",
            Self::Ping => "ping",
            Self::Pong => "pong",
            Self::Other => "other",
        }
    }
}

pub struct GrpcListenerMetrics;

impl GrpcListenerMetrics {
    /// Call once at startup — see [`GrpcBufferMetrics::register_descriptions`].
    pub(crate) fn register_descriptions() {
        describe_counter!(
            UPDATES_RECEIVED,
            "Yellowstone updates received, labelled by kind"
        );
        describe_counter!(
            ADAPTER_FAILURES,
            "Transactions that could not be translated into the neutral shape,              labelled by the kind of malformation"
        );
        describe_counter!(
            TRANSACTIONS_EMITTED,
            "Timestamped transactions handed to the rest of the pipeline"
        );
        describe_counter!(
            DOWNSTREAM_FULL,
            "Times the downstream channel was full and the stream was slowed              to the consumer's speed"
        );
    }

    pub(crate) fn record_update(kind: UpdateKind) {
        counter!(UPDATES_RECEIVED, "kind" => kind.as_str()).increment(1);
    }

    pub(crate) fn record_adapter_failure(kind: &'static str) {
        counter!(ADAPTER_FAILURES, "kind" => kind).increment(1);
    }

    pub(crate) fn record_emitted() {
        counter!(TRANSACTIONS_EMITTED).increment(1);
    }

    pub(crate) fn record_downstream_full() {
        counter!(DOWNSTREAM_FULL).increment(1);
    }
}
