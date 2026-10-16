//! What the detectors count while they evaluate: the pools they were handed, and the ones they
//! declined, with the reason; the skip counter's `# HELP` is built from [`SkipReason`].

use metrics::{counter, describe_counter};

const SKIPPED_TOTAL: &str = "yog_signals_skipped_total";
const CONSIDERED_TOTAL: &str = "yog_signals_considered_total";

/// Why a detector declined to evaluate a pool, the `reason` label of [`SKIPPED_TOTAL`].
/// ⚠️ The macro declares variants, labels and `ALL` in one place, which the `# HELP` text reads:
/// a variant cannot be added without being described.
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
    /// The window was not entirely valuable: some hour carried a token with no usable price.
    Unpriced => "unpriced",
    /// The pool's current TVL could not be priced.
    NoTvl => "no_tvl",
    /// An input was older than its freshness gate.
    Stale => "stale",
    /// No `sqrt_price` decoder for that protocol: missing code, not data, hence apart from `Undecodable`.
    NoDecoder => "no_decoder",
    /// A ratio that will not compute: a zero price, or a magnitude overflow on an extreme pair.
    Undecodable => "undecodable",
}

/// Counters for what a detector saw on a tick.
pub(crate) struct DetectorMetrics;

impl DetectorMetrics {
    /// Register human-readable descriptions. Call once, before any tick.
    pub(crate) fn register_descriptions() {
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

    /// Record one pool a detector declined to evaluate.
    /// ⚠️ A number, not a silence: otherwise degrading price coverage looks like a calm market.
    pub(crate) fn record_skipped(detector: &'static str, reason: SkipReason) {
        counter!(SKIPPED_TOTAL, "detector" => detector, "reason" => reason.as_str()).increment(1);
    }

    /// Record how many pools a tick was handed, before any guard.
    /// ⚠️ The denominator of `skipped`: per tick, it would move with the pool count, not with coverage.
    pub(crate) fn record_considered(detector: &'static str, count: usize) {
        counter!(CONSIDERED_TOTAL, "detector" => detector).increment(count as u64);
    }
}
