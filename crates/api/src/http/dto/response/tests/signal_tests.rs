use yog_core::domain;

use super::SeverityResponse;
use crate::testing::wire_tag;

#[test]
fn severity_matches_the_domain() {
    use domain::Severity::*;
    for v in [Info, Warning, Critical] {
        assert_eq!(wire_tag(SeverityResponse::from(v)), v.as_str());
    }
}
