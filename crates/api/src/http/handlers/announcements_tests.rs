use axum::http::StatusCode;

use crate::testing::get;

/// The control: the collection reaches its handler, with and without a status.
#[tokio::test]
async fn the_announcements_are_mounted() {
    for uri in [
        "/api/announcements",
        "/api/announcements?status=open&limit=200",
        "/api/announcements?status=upcoming",
        "/api/announcements?status=closed",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
}

/// Mutation: the status read as a free string, or a cursor decoded without its status — an
/// unknown status answered empty, or a page of another status followed.
#[tokio::test]
async fn an_unknown_status_or_a_cursor_of_another_is_refused() {
    use yog_core::{
        Cursor,
        domain::{AnnouncementCursor, AnnouncementStatus},
    };
    let open =
        crate::http::cursor::encode_cursor(&Cursor::Announcement(Box::new(AnnouncementCursor {
            status: Some(AnnouncementStatus::Open),
            starts_at: crate::testing::ts(1_000),
            id: 1,
        })))
        .unwrap();
    for uri in [
        "/api/announcements?status=active".to_owned(),
        format!("/api/announcements?cursor={open}"),
        format!("/api/announcements?status=closed&cursor={open}"),
    ] {
        let (status, content_type, detail) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {detail}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: the status of the query not passed on to the read — every announcement listed
/// under a status.
#[tokio::test]
async fn the_status_of_the_query_reaches_the_read() {
    let (status, _, body) = get("/api/announcements?status=closed").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(page["items"][0]["status"], "closed", "{page}");
}
