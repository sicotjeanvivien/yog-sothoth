//! Metrics for the gRPC ingestion path: the slot/time buffer's, and the listener's.

use std::sync::atomic::{AtomicU64, Ordering};

use metrics::{counter, describe_counter, describe_gauge, gauge};

/// Transactions dropped because their slot never received a block time.
/// ⚠️ Counts transactions, not adapter payloads; staying at zero says nothing of the margin
/// left under the bounds, which the two peaks below measure.
const TRANSACTIONS_EVICTED: &str = "yog_indexer_grpc_untimestamped_transactions_total";

/// The most payloads waiting at once since start, against `MAX_PENDING_PAYLOADS`.
const PENDING_PAYLOADS_PEAK: &str = "yog_indexer_grpc_pending_payloads_peak";

/// The furthest the oldest waiting slot has trailed the head since start, against `MAX_PENDING_SLOTS`.
const PENDING_SLOT_LAG_PEAK: &str = "yog_indexer_grpc_pending_slot_lag_peak";

/// The process-wide peaks of what the buffer holds, each exported as a gauge when it rises.
/// ⚠️ Not held by the buffer: it dies with its session, and the next one would lower the gauges.
pub(crate) struct WaitingPeaks {
    payloads: AtomicU64,
    slot_lag: AtomicU64,
}

/// The peaks every buffer of the process reports to.
pub(crate) static WAITING_PEAKS: WaitingPeaks = WaitingPeaks::new();

impl WaitingPeaks {
    pub(crate) const fn new() -> Self {
        Self {
            payloads: AtomicU64::new(0),
            slot_lag: AtomicU64::new(0),
        }
    }

    /// Take what the buffer holds now; a gauge moves only when its peak rises.
    pub(crate) fn record(&self, payloads: usize, slot_lag: u64) {
        raise(&self.payloads, payloads as u64, PENDING_PAYLOADS_PEAK);
        raise(&self.slot_lag, slot_lag, PENDING_SLOT_LAG_PEAK);
    }
}

fn raise(peak: &AtomicU64, value: u64, gauge_name: &'static str) {
    if peak.fetch_max(value, Ordering::Relaxed) < value {
        gauge!(gauge_name).set(value as f64);
    }
}

/// Which bound forced the eviction, on every increment of [`TRANSACTIONS_EVICTED`].
/// ⚠️ Unlabelled, a payload-bound eviction would read as "the slot ceiling is too small", and
/// raising it would change nothing.
#[derive(Debug, Clone, Copy)]
pub(crate) enum EvictionReason {
    /// More than `MAX_PENDING_SLOTS` behind the head: its block-meta is not late, it is not coming.
    SlotBound,
    /// Too many payloads held across all pending slots.
    PayloadBound,
    /// The block-meta came with **no** `block_time`, or a fork abandoned the slot: nothing will resolve it.
    /// ⚠️ Counted `slot_bound`, it would ask for a wider window, which changes nothing.
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
    /// Call once at startup to register the descriptions with the Prometheus exporter; without it
    /// the counters export with no HELP text.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TRANSACTIONS_EVICTED,
            "Transactions dropped because their slot never received a block time, \
             labelled by the bound that forced it"
        );
        describe_gauge!(
            PENDING_PAYLOADS_PEAK,
            "The most transactions waiting for their slot's block time at once since start"
        );
        describe_gauge!(
            PENDING_SLOT_LAG_PEAK,
            "The most slots the oldest waiting transaction has trailed the stream's head since start"
        );
        // ⚠️ Set at 0 here, or a peak never raised is an absent series, read as unwired.
        gauge!(PENDING_PAYLOADS_PEAK).set(0.0);
        gauge!(PENDING_SLOT_LAG_PEAK).set(0.0);
    }

    pub(crate) fn record_evicted(count: usize, reason: EvictionReason) {
        counter!(TRANSACTIONS_EVICTED, "reason" => reason.as_str()).increment(count as u64);
    }
}

/// Updates taken off the stream, by what they were.
/// ⚠️ Read to check the subscription filters: a provider ignoring `vote` or `failed` shows here as
/// a `transaction` rate far above the pool activity, and nowhere else.
const UPDATES_RECEIVED: &str = "yog_indexer_grpc_updates_total";

/// Transactions dropped before reaching the pipeline, by reason; the stream keeps running.
/// ⚠️ Two different faults share it: an untranslatable update (`from_grpc`) and an unroutable one
/// (the request); the label tells them apart.
const DROPPED_TRANSACTIONS: &str = "yog_indexer_grpc_dropped_transactions_total";

/// Transactions handed downstream, timestamped.
const TRANSACTIONS_EMITTED: &str = "yog_indexer_grpc_transactions_emitted_total";

/// Times the downstream channel was full when a transaction was ready.
/// ⚠️ A back-pressure counter, not a loss counter, unlike the dispatcher's `downstream_saturated`:
/// the listener waits instead of dropping.
const DOWNSTREAM_FULL: &str = "yog_indexer_grpc_downstream_full_total";

/// Server pings this client could not answer, by reason. A provider may close
/// a connection that does not answer, so this can precede a reconnection.
const PING_REPLIES_UNSENT: &str = "yog_indexer_grpc_ping_replies_unsent_total";

/// Attempts ended because the server said nothing for the stall timeout, by [`StallSite`].
/// ⚠️ A stall retried with a mark is a billed replay.
const STALLS: &str = "yog_indexer_grpc_stalls_total";

/// What an update was, for [`UPDATES_RECEIVED`].
#[derive(Debug, Clone, Copy)]
pub(crate) enum UpdateKind {
    Transaction,
    BlockMeta,
    /// A server keep-alive, counted and answered with a ping-only request.
    Ping,
    /// The server's answer to one of ours. ⚠️ Rises with `ping` while answers get through;
    /// [`PING_REPLIES_UNSENT`] says why when they do not.
    Pong,
    /// Anything the subscription did not ask for: non-zero means the request and the reader disagree.
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

/// Why a transaction was dropped, for [`DROPPED_TRANSACTIONS`].
#[derive(Debug, Clone, Copy)]
pub(crate) enum DropReason {
    /// The two `CoreError` kinds `from_grpc` returns: an absent field, a value that will not parse.
    MissingField,
    ParseError,
    /// A `from_grpc` failure of another kind: a label, not `unreachable!`, so a future one stays countable.
    OtherMalformation,
    /// The update matched no protocol filter: the request and this reader disagree, which is
    /// fixed in `subscription`, not in the adapter.
    Unroutable,
}

impl DropReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::ParseError => "parse_error",
            Self::OtherMalformation => "other_malformation",
            Self::Unroutable => "unroutable",
        }
    }
}

/// Why a ping answer did not leave, for [`PING_REPLIES_UNSENT`]: a transport
/// that stopped draining, or a request stream that is gone.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PingReplyFailure {
    /// The outbound half is full: the transport is not taking what is sent.
    OutboundFull,
    /// The outbound half is closed: the request stream has ended.
    OutboundClosed,
}

impl PingReplyFailure {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OutboundFull => "outbound_full",
            Self::OutboundClosed => "outbound_closed",
        }
    }
}

/// Where a stall happened, for [`STALLS`]: a front end that never answers
/// `subscribe` shows nothing in [`UPDATES_RECEIVED`]; a stopped data path
/// shows `ping` rising over a flat `block_meta`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StallSite {
    /// `subscribe` was never answered.
    Subscribe,
    /// The stream was open and no block-meta came.
    Stream,
}

impl StallSite {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Subscribe => "subscribe",
            Self::Stream => "stream",
        }
    }

    /// What the attempt's error says happened.
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Subscribe => "subscribe never answered",
            Self::Stream => "no block-meta from the stream",
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
            DROPPED_TRANSACTIONS,
            "Transactions dropped before reaching the pipeline: malformed for \
             the adapter, or matching no protocol filter"
        );
        describe_counter!(
            TRANSACTIONS_EMITTED,
            "Timestamped transactions handed to the rest of the pipeline"
        );
        describe_counter!(
            DOWNSTREAM_FULL,
            "Times the downstream channel was full and the stream was slowed \
             to the consumer's speed"
        );
        describe_counter!(
            STALLS,
            "Attempts ended because the server said nothing for the stall \
             timeout, labelled by where: no answer to subscribe, or no \
             block-meta on an open stream"
        );
        describe_counter!(
            PING_REPLIES_UNSENT,
            "Server pings that could not be answered, labelled by why the \
             answer did not leave"
        );
    }

    pub(crate) fn record_update(kind: UpdateKind) {
        counter!(UPDATES_RECEIVED, "kind" => kind.as_str()).increment(1);
    }

    pub(crate) fn record_dropped(reason: DropReason) {
        counter!(DROPPED_TRANSACTIONS, "reason" => reason.as_str()).increment(1);
    }

    pub(crate) fn record_emitted() {
        counter!(TRANSACTIONS_EMITTED).increment(1);
    }

    pub(crate) fn record_downstream_full() {
        counter!(DOWNSTREAM_FULL).increment(1);
    }

    pub(crate) fn record_ping_reply_unsent(failure: PingReplyFailure) {
        counter!(PING_REPLIES_UNSENT, "reason" => failure.as_str()).increment(1);
    }

    pub(crate) fn record_stall(site: StallSite) {
        counter!(STALLS, "site" => site.as_str()).increment(1);
    }
}
