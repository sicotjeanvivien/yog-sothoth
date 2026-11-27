use crate::domain::FieldOutOfRange;

/// Why a stored fee curve was not rebuilt on read, so `fee_scheduler` is `None` for a curve
/// that exists. Six NULL columns are not one: that curve was never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeSchedulerDrop {
    /// A stored value does not fit its domain type.
    OutOfRange(FieldOutOfRange),

    /// Some columns NULL, others not: a corrupt row.
    PartlyNull,
}
