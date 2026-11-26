use rust_decimal::Decimal;

/// The LPs' fees of DAMM v2, split between those claimed and those compounded into the pool;
/// each `None` where not measured. Their sum is `lp_fees_usd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2LpFeeSplit {
    pub claiming_fees_usd: Option<Decimal>,
    pub compounding_fees_usd: Option<Decimal>,
}
