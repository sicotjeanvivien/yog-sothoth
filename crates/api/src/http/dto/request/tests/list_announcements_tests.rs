use axum::{extract::Query, http::Uri};
use yog_core::domain::AnnouncementStatus;

use super::{AnnouncementsParams, ListAnnouncementsRequest};

fn parse(query: &str) -> ListAnnouncementsRequest {
    let uri: Uri = format!("/api/announcements{query}").parse().unwrap();
    let Query(params) = Query::<AnnouncementsParams>::try_from_uri(&uri).unwrap();
    ListAnnouncementsRequest::parse(params).unwrap()
}

/// Mutation: a status of the query mapped to another — the list of one status read as another's.
#[test]
fn each_status_of_the_query_reaches_the_read() {
    assert_eq!(parse("").status, None);
    assert_eq!(
        parse("?status=upcoming").status,
        Some(AnnouncementStatus::Upcoming)
    );
    assert_eq!(parse("?status=open").status, Some(AnnouncementStatus::Open));
    assert_eq!(
        parse("?status=closed").status,
        Some(AnnouncementStatus::Closed)
    );
}
