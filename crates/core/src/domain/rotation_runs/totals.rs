use rust_decimal::Decimal;

/// The runs that started in a period, and their volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationRunTotals {
    pub runs: i64,
    /// The distinct pools the runs are of.
    pub pools: i64,
    /// The runs not extinct yet.
    pub in_progress: i64,
    /// From start to end, `None` without a run.
    pub median_duration_seconds: Option<i64>,
    pub p90_duration_seconds: Option<i64>,
    pub volume_usd: Decimal,
    /// `None` while no run's first hour ended before the period does.
    pub first_hour: Option<RunsFirstHour>,
}

/// The runs whose first hour ended before their period did, their volume, and its part in that hour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunsFirstHour {
    pub runs: i64,
    pub volume_usd: Decimal,
    pub in_first_hour_usd: Decimal,
}
