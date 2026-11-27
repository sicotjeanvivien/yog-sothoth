use yog_core::amm::damm_v2::{BaseFeeKind, FeeSchedulerParams};
use yog_core::domain::FieldOutOfRange;

use super::scheduler_columns;

fn curve() -> FeeSchedulerParams {
    FeeSchedulerParams {
        cliff_fee_numerator: 500_000_000,
        number_of_period: 144,
        period_frequency: 600,
        reduction_factor: 3_194_444,
        activation_point: 1_785_180_416,
        activation_type: 1,
        kind: BaseFeeKind::SchedulerLinear,
    }
}

#[test]
fn a_curve_that_fits_keeps_every_value() {
    let c = scheduler_columns(&curve()).expect("fits");

    assert_eq!(
        (
            c.cliff_fee_numerator,
            c.number_of_period,
            c.period_frequency,
            c.reduction_factor,
            c.activation_point,
            c.activation_type
        ),
        (500_000_000, 144, 600, 3_194_444, 1_785_180_416, 1)
    );
}

/// Mutation: a field reported under another's name, or the overflow wrapped instead of refused.
#[test]
fn a_value_beyond_bigint_names_its_field() {
    type Overflow = fn(&mut FeeSchedulerParams);
    let cases: [(&str, Overflow); 4] = [
        ("cliff_fee_numerator", |c| c.cliff_fee_numerator = u64::MAX),
        ("period_frequency", |c| c.period_frequency = u64::MAX),
        ("reduction_factor", |c| c.reduction_factor = u64::MAX),
        ("activation_point", |c| c.activation_point = u64::MAX),
    ];
    for (field, overflow) in cases {
        let mut c = curve();
        overflow(&mut c);

        assert_eq!(
            scheduler_columns(&c).err(),
            Some(FieldOutOfRange { field }),
            "{field}"
        );
    }
}
