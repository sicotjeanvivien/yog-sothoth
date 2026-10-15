use yog_core::domain;

use super::freshness_status_response::FreshnessStatusResponse;
use crate::testing::wire_tag;

#[test]
fn freshness_matches_the_domain() {
    use domain::FreshnessStatus::*;
    for v in [Live, Delayed, Stale] {
        assert_eq!(wire_tag(FreshnessStatusResponse::from(v)), v.as_str());
    }
}
