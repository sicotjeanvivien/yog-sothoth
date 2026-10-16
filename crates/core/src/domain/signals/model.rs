//! A signal: a conclusion a detector emits on observed data, not an on-chain
//! event. One generic `signals` table, discriminated by `detector` and `protocol`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// How much attention a signal warrants; mirrors the `signals.severity` CHECK.
/// ⚠️ The declaration order is the escalation order the engine's dedup compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    /// Stable tag of the `severity` column.
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
    /// The producing detector's [`SignalDetector::name`](crate::domain::SignalDetector::name).
    pub detector: String,

    /// The protocol of the pool this signal is about.
    pub protocol: Protocol,

    /// The pool the signal concerns.
    pub pool_address: Pubkey,

    /// How much attention it warrants.
    pub severity: Severity,

    /// The metric value that crossed the threshold, in the detector's own unit.
    pub value: Decimal,

    /// The threshold crossed, when the detector has a single scalar one.
    pub threshold: Option<Decimal>,

    /// Optional human-readable summary for feeds and notifications.
    pub message: Option<String>,

    /// The tick instant, shared by every signal of one evaluation.
    pub triggered_at: DateTime<Utc>,
}

/// A persisted signal and its storage `id`, the tie-breaker of the feed ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalRecord {
    /// Storage-assigned identity, unique within the table.
    pub id: i64,
    /// The signal as emitted by its detector.
    pub signal: Signal,
}
