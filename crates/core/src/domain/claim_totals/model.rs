//! The claims of positions' fees and rewards, each valued once by yog-analytic, summed by protocol.

use rust_decimal::Decimal;

/// A protocol's claims of one kind over a period. The USD sum skips the claims not valued,
/// which `claims - claims_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTotals {
    pub claims: i64,
    pub claims_valued: i64,
    pub amount_usd: Decimal,
    /// Distinct over the period: it does not add up across periods.
    pub owners: i64,
}

/// Which claims: a position's fees, or its rewards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimKind {
    PositionFee,
    Reward,
}
