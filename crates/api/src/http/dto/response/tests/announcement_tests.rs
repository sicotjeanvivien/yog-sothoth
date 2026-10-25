use yog_core::domain;

use chrono::{TimeZone, Utc};
use serde_json::json;

use super::AnnouncementEntryResponse;
use super::kind_response::AnnouncementKindResponse;
use super::severity_response::AnnouncementSeverityResponse;
use super::status_response::AnnouncementStatusResponse;
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

#[test]
fn status_matches_the_domain() {
    use domain::AnnouncementStatus::*;
    for v in [Upcoming, Open, Closed] {
        assert_eq!(wire_tag(AnnouncementStatusResponse::from(v)), v.as_str());
    }
}

/// Mutation: two fields swapped in the conversion, or the status left out — an announcement
/// served with another's window, or without where it stands.
#[test]
fn an_entry_says_every_field_and_its_status() {
    let entry = domain::AnnouncementEntry {
        announcement: domain::Announcement {
            id: 7,
            kind: domain::AnnouncementKind::Release,
            severity: domain::AnnouncementSeverity::Warning,
            message: "m".to_owned(),
            link_url: Some("/changelog".to_owned()),
            starts_at: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
            ends_at: Some(Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap()),
        },
        status: domain::AnnouncementStatus::Closed,
    };

    let wire = serde_json::to_value(AnnouncementEntryResponse::from(entry)).unwrap();

    assert_eq!(
        wire,
        json!({
            "id": 7, "kind": "release", "severity": "warning", "message": "m",
            "linkUrl": "/changelog", "startsAt": "2026-10-25T12:00:00Z",
            "endsAt": "2026-10-26T12:00:00Z", "status": "closed"
        })
    );
}
