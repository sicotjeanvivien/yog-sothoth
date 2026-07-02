//! Signal domain model.
//!
//! A signal is a *conclusion* emitted by a [`SignalDetector`] when a
//! condition on the observed data becomes true — not a raw on-chain
//! event. Its shape is uniform across protocols, so it lives in a single
//! generic table (`signals`), discriminated by `detector` + `protocol`.
//! Pure domain type; no persistence backend leaks in here.
//!
//! [`SignalDetector`]: crate::domain::SignalDetector

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// How much attention a signal warrants. Closed, stable set → an enum,
/// mirrored one-to-one by the `signals.severity` CHECK constraint.
///
/// `Ord` follows the declaration order (`Info < Warning < Critical`), which
/// is exactly the escalation order the engine's dedup compares against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    /// Stable snake_case tag, as persisted in the `severity` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Critical => "critical",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Severity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "info" => Ok(Severity::Info),
            "warning" => Ok(Severity::Warning),
            "critical" => Ok(Severity::Critical),
            _ => Err(()),
        }
    }
}

/// A single emitted signal — one row of the `signals` table.
#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    /// Which detector fired. A stable snake_case tag equal to the
    /// producing detector's [`SignalDetector::name`]. A plain `String`
    /// (not a central enum) on purpose: detectors are the product's
    /// high-churn, open-ended surface — a shared enum would re-tax the
    /// one action done most often, the same reason `signals` is not
    /// split per-detector.
    ///
    /// [`SignalDetector::name`]: crate::domain::SignalDetector::name
    pub detector: String,

    /// The protocol of the pool this signal is about. Closed set → the
    /// shared [`Protocol`] enum, persisted as its snake_case tag.
    pub protocol: Protocol,

    /// The pool the signal concerns.
    pub pool_address: Pubkey,

    /// How much attention it warrants.
    pub severity: Severity,

    /// The metric value that crossed the threshold (units are the
    /// detector's own — a ratio, a percentage, a USD amount). Exact
    /// fixed-point, like [`crate::domain::TokenPrice::price_usd`].
    pub value: Decimal,

    /// The threshold that was crossed, kept for traceability. Optional:
    /// some detectors have no single scalar threshold.
    pub threshold: Option<Decimal>,

    /// Optional human-readable summary for feeds and notifications.
    pub message: Option<String>,

    /// The tick instant at which the signal was raised. Shared by every
    /// signal of one evaluation (see [`EvalContext`]).
    ///
    /// [`EvalContext`]: crate::domain::EvalContext
    pub triggered_at: DateTime<Utc>,
}

/// A persisted signal, as read back from storage.
///
/// Wraps the [`Signal`] payload with its storage identity: `id` only
/// exists once the row is inserted (`BIGSERIAL`), so it has no place on
/// the write-side [`Signal`] — but the read side needs it as the
/// deterministic tie-breaker of the feed ordering (see
/// [`SignalCursor`]).
///
/// [`SignalCursor`]: crate::domain::SignalCursor
#[derive(Debug, Clone, PartialEq)]
pub struct SignalRecord {
    /// Storage-assigned identity, unique within the table.
    pub id: i64,
    /// The signal as emitted by its detector.
    pub signal: Signal,
}
