use rust_decimal::Decimal;

/// A bar's volume split by the rotation runs: `in_runs_usd` is `None` where the runs do not
/// cover the bar whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSplit {
    pub volume_usd: Option<Decimal>,
    pub in_runs_usd: Option<Decimal>,
}

impl RunSplit {
    /// The volume outside the runs, never under 0; `None` as soon as one term is unknown.
    pub fn outside_runs_usd(&self) -> Option<Decimal> {
        Some((self.volume_usd? - self.in_runs_usd?).max(Decimal::ZERO))
    }
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
