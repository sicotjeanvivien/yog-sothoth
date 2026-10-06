//! Metrics emitted by the network status reporter.
//!
//! One family, and it exists because a failed tick no longer stops anything.
//! Until 11 September 2026 the first failed `getSlot` took the daemon down with
//! it, which was loud; now it is skipped, and this counter is what remains of
//! the noise.

use metrics::{counter, describe_counter};

const TICK_FAILURES: &str = "yog_indexer_network_status_tick_failures_total";

pub(crate) struct NetworkStatusReporterMetrics;

impl NetworkStatusReporterMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_FAILURES,
            "Network status ticks that recorded no snapshot and were skipped \
             (label: reason — `rpc` or `persistence`)"
        );
    }

    /// A tick that recorded nothing.
    ///
    /// ⚠️ **Not a sign that ingestion is down.** The probe and the data path
    /// share an endpoint and a network, so they usually fail together — but
    /// the decision to stop belongs to the data path alone, and this series
    /// rising while `yog_indexer_*` keeps advancing says the probe is the only
    /// thing hurting.
    ///
    /// ⚠️ **And it is the only signal there is** — raised in review of PR #141.
    /// A failing probe freezes `network_status.observed_at`, and nothing
    /// surfaces that freeze: the dashboard parses `observedAt` and never reads
    /// it (`web/src/lib/api/schema/network-status.ts` is its only occurrence in
    /// `web/src`), and the freshness dot beside the slot is computed from the
    /// last *indexed event* — `NetworkStatusService::get_status` — which keeps
    /// advancing precisely when the probe alone is down. The sidebar therefore
    /// shows a stale slot and a stale latency under a pulsing "live" badge.
    /// Whoever gives the freeze a reader — a derived field in the DTO, or the
    /// dashboard reading `observedAt` — closes that; until then this counter is
    /// what an operator has, and the project carries no alerting rule.
    pub(crate) fn record_tick_failure(reason: &'static str) {
        counter!(TICK_FAILURES, "reason" => reason).increment(1);
    }
}
