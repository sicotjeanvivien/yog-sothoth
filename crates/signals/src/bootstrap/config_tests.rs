use super::*;

#[test]
fn ladder_accepts_threshold_below_critical() {
    assert!(validate_ladder("KEY", Decimal::new(5, 2), Decimal::new(2, 1)).is_ok());
}

#[test]
fn ladder_rejects_threshold_at_or_above_critical() {
    // Equal: Warning would be unreachable.
    assert!(validate_ladder("KEY", Decimal::new(2, 1), Decimal::new(2, 1)).is_err());
    // Above: every emitted signal would be Critical.
    assert!(validate_ladder("KEY", Decimal::new(3, 1), Decimal::new(2, 1)).is_err());
}
