//! Request DTO for `GET /api/announcements`.

mod status_param;

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::{AnnouncementCursor, AnnouncementStatus};

use crate::http::{cursor::decode_announcement_cursor, error::ApiError};

use status_param::AnnouncementStatusParam;

/// Every announcement, or those of one status, forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnnouncementsParams {
    #[param(inline)]
    status: Option<AnnouncementStatusParam>,
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListAnnouncementsRequest {
    pub(crate) status: Option<AnnouncementStatus>,
    pub(crate) cursor: Option<AnnouncementCursor>,
    pub(crate) limit: i64,
}

impl ListAnnouncementsRequest {
    pub(crate) fn parse(params: AnnouncementsParams) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let status = params.status.map(Into::into);
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw)
                    .and_then(|raw| decode_announcement_cursor(raw, status))
            })
            .transpose()?;
        Ok(Self {
            status,
            cursor,
            limit: params.limit,
        })
    }
}

#[cfg(test)]
#[path = "tests/list_announcements_tests.rs"]
mod tests;
