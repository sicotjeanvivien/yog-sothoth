//! Metrics emitted by the signal engine.
//!
//! Mirrors the other daemons: cumulative counters exposed on the
//! Prometheus `/metrics` endpoint the binary installs. The lib emits
//! through the `metrics` facade (a no-op if no recorder is installed, so
//! unit tests need no exporter); the binary installs the exporter and
//! calls [`EngineMetrics::register_descriptions`] once at startup.

use metrics::{counter, describe_counter};

const TICK_TOTAL: &str = "yog_signals_tick_total";
const EMITTED_TOTAL: &str = "yog_signals_emitted_total";
const SKIPPED_TOTAL: &str = "yog_signals_skipped_total";
const CONSIDERED_TOTAL: &str = "yog_signals_considered_total";

/// Why a detector declined to evaluate a pool — the `reason` label of
/// [`SKIPPED_TOTAL`].
///
/// An enum rather than string literals at the call sites, and the `# HELP` text
/// is built from [`SkipReason::ALL`] rather than retyped. The label set used to
/// be spelled out in three places — five call sites, the `describe_counter!`
/// string and the crate README — and within a single PR the description drifted:
/// `no_decoder` shipped while the HELP text still listed four values. Prometheus
/// shows that string to whoever is debugging, so the copy that rotted was the
/// one closest to the person who needed it. The variants are now the definition;
/// adding one without describing it is no longer expressible.
/// Declares the variants, their label strings and the exhaustive list in ONE
/// place. A hand-written `ALL` array would have left the same gap one rung
/// lower: add a variant, let the `match` force you to give it a label, forget
/// the array, and the `# HELP` text is silently short again.
macro_rules! skip_reasons {
    ($( $(#[$meta:meta])* $variant:ident => $label:literal ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum SkipReason {
            $( $(#[$meta])* $variant ),+
        }

        impl SkipReason {
            /// Every variant, in the order the `# HELP` text lists them.
            const ALL: &'static [Self] = &[ $( Self::$variant ),+ ];

            const fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $label ),+ }
            }
        }
    };
}

skip_reasons! {
    /// The window was not entirely valuable — some hour in it carried an amount
    /// in a token with no usable price.
    Unpriced => "unpriced",
    /// The pool's current TVL could not be priced.
    NoTvl => "no_tvl",
    /// An input was older than its freshness gate.
    Stale => "stale",
    /// No `sqrt_price` decoder shipped for that protocol — a backlog item rather
    /// than a data problem, which is why it is not lumped in with `Undecodable`.
    NoDecoder => "no_decoder",
    /// A ratio that will not compute: a zero price, or a magnitude overflow on
    /// an extreme pair.
    Undecodable => "undecodable",
}

/// Counters for the engine's per-detector poll loops.
pub struct EngineMetrics;

impl EngineMetrics {
    /// Register human-readable descriptions. Call once, before any tick.
    pub fn register_descriptions() {
        describe_counter!(
            TICK_TOTAL,
            "Detector ticks completed (labels: detector, \
             outcome=ok|suppressed|eval_failed|dedup_failed|persist_failed)"
        );
        describe_counter!(
            EMITTED_TOTAL,
            "Signals persisted, cumulative (label: detector)"
        );
        let reasons = SkipReason::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join("|");
        describe_counter!(
            SKIPPED_TOTAL,
            format!(
                "Pools a detector declined to evaluate, cumulative \
                 (labels: detector, reason={reasons})"
            )
        );
        describe_counter!(
            CONSIDERED_TOTAL,
            "Pools a detector was handed, cumulative (label: detector) — the \
             denominator SKIPPED_TOTAL needs to mean anything"
        );
    }

    /// Record one completed tick with its outcome.
    pub(crate) fn record_tick(detector: &'static str, outcome: &'static str) {
        counter!(TICK_TOTAL, "detector" => detector, "outcome" => outcome).increment(1);
    }

    /// Record signals successfully persisted on a tick.
    pub(crate) fn record_emitted(detector: &'static str, count: usize) {
        counter!(EMITTED_TOTAL, "detector" => detector).increment(count as u64);
    }

    /// Record one pool a detector declined to evaluate.
    ///
    /// A pool skipped for want of a price must be a **number, not a silence**:
    /// it is the only way to tell "nothing is happening" from "we stopped being
    /// able to see". Emitting no signal is the right answer to an unvaluable
    /// pool (`.project` ticket 08) — staying quiet about how often that happens
    /// is not, because a degrading price coverage then looks exactly like a
    /// calm market.
    pub(crate) fn record_skipped(detector: &'static str, reason: SkipReason) {
        counter!(SKIPPED_TOTAL, "detector" => detector, "reason" => reason.as_str()).increment(1);
    }

    /// Record how many pools a tick was handed, before any guard.
    ///
    /// Without it `skipped` has no denominator: it counts per POOL while
    /// `tick_total` counts per TICK, so `skipped / tick` reads as "pools skipped
    /// per run" and moves with the pool count rather than with price coverage —
    /// an alert built on it says nothing. `skipped / considered` is the share of
    /// the universe a detector cannot see, which is the thing worth watching.
    pub(crate) fn record_considered(detector: &'static str, count: usize) {
        counter!(CONSIDERED_TOTAL, "detector" => detector).increment(count as u64);
    }
}
