use rust_decimal::Decimal;

/// The liquidity deposited at the pools' creation, added and removed over a period or a bucket.
/// The USD sums skip the moves not valued, which `moves - moves_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityFlows {
    pub created_usd: Decimal,
    pub added_usd: Decimal,
    pub removed_usd: Decimal,
    pub moves: i64,
    pub moves_valued: i64,
}

/// A period's liquidity, and the distinct owners of its moves: a count no sum of hours gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityTotals {
    pub flows: LiquidityFlows,
    pub owners: i64,
}
