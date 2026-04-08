use chrono::{DateTime, Utc};

/// Whether liquidity was added or removed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LiquidityEventType {
    Add,
    Remove,
}

impl LiquidityEventType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            LiquidityEventType::Add => "add",
            LiquidityEventType::Remove => "remove",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "add" => Some(LiquidityEventType::Add),
            "remove" => Some(LiquidityEventType::Remove),
            _ => None,
        }
    }
}

/// A parsed liquidity add or remove event — DB representation.
#[derive(Debug, Clone)]
pub(crate) struct LiquidityEvent {
    /// Pool address (base58).
    pub(crate) pool_address: String,
    /// Transaction signature (base58).
    pub(crate) signature: String,
    pub(crate) event_type: LiquidityEventType,
    /// Amount of token A in native units.
    pub(crate) amount_a: u64,
    /// Amount of token B in native units.
    pub(crate) amount_b: u64,
    /// Block timestamp.
    pub(crate) timestamp: DateTime<Utc>,
}
