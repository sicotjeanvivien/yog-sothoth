use super::FrozenIntervalParam;
use crate::application::BucketPeriod;

/// Mutation: the two steps swapped — `interval=1h` read as the days, `1d` as the hours.
#[test]
fn a_frozen_interval_reads_its_buckets() {
    assert_eq!(
        BucketPeriod::from(FrozenIntervalParam::Hour),
        BucketPeriod::Hour
    );
    assert_eq!(
        BucketPeriod::from(FrozenIntervalParam::Day),
        BucketPeriod::Day
    );
}
