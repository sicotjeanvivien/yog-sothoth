use yog_core::domain;

use super::kind_response::AnnouncementKindResponse;
use super::severity_response::AnnouncementSeverityResponse;
use crate::testing::wire_tag;

#[test]
fn kind_matches_the_domain() {
    use domain::AnnouncementKind::*;
    for v in [Maintenance, Incident, Release, Beta] {
        assert_eq!(wire_tag(AnnouncementKindResponse::from(v)), v.as_str());
    }
}

#[test]
fn severity_matches_the_domain() {
    use domain::AnnouncementSeverity::*;
    for v in [Info, Warning, Critical] {
        assert_eq!(wire_tag(AnnouncementSeverityResponse::from(v)), v.as_str());
    }
}
