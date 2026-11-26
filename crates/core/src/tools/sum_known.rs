use rust_decimal::Decimal;

/// The terms added, `None` as soon as one is unknown or their sum overflows.
pub(crate) fn sum_known(terms: impl IntoIterator<Item = Option<Decimal>>) -> Option<Decimal> {
    terms
        .into_iter()
        .try_fold(Decimal::ZERO, |sum, term| sum.checked_add(term?))
}
