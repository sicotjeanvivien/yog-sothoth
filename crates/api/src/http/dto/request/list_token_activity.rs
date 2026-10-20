//! Request DTO for `GET /api/tokens/activity`.

use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use yog_core::domain::{TokenActivityCursor, Window};

use super::window_param::WindowParam;
use crate::http::{cursor::decode_token_activity_cursor, error::ApiError};

/// The only order: the tokens most traded first.
#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SortParam {
    #[default]
    Volume,
}

/// Query params for `GET /api/tokens/activity`: a window, the order, and
/// forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct TokenActivityQuery {
    #[serde(default)]
    #[param(inline)]
    window: WindowParam,
    #[serde(default)]
    #[param(inline)]
    sort: SortParam,
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListTokenActivityRequest {
    pub(crate) window: Window,
    pub(crate) cursor: Option<TokenActivityCursor>,
    pub(crate) limit: i64,
}

impl ListTokenActivityRequest {
    pub(crate) fn parse(query: TokenActivityQuery) -> Result<Self, ApiError> {
        super::validate_limit(query.limit)?;
        let SortParam::Volume = query.sort;
        let cursor = query
            .cursor
            .as_deref()
            .map(|raw| super::non_empty("cursor", raw).and_then(decode_token_activity_cursor))
            .transpose()?;
        Ok(Self {
            window: query.window.into(),
            cursor,
            limit: query.limit,
        })
    }
}
